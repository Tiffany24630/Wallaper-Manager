#!/usr/bin/env python3
"""Small, dependency-free Lumina account/sync service for local or self-hosted use."""
import argparse
import hashlib
import hmac
import ipaddress
import json
import os
import re
import secrets
import sqlite3
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlparse

TOKEN_TTL = 60 * 60 * 24 * 30
MAX_BODY = 2 * 1024 * 1024
MAX_FILE_SIZE = int(os.environ.get("LUMINA_MAX_FILE_BYTES", str(100 * 1024 * 1024)))
CHUNK_SIZE = 1024 * 1024
ALLOWED_FILES = {
    "image/png": ".png", "image/jpeg": ".jpg", "image/webp": ".webp", "image/bmp": ".bmp",
    "image/x-icon": ".cur", "application/octet-stream": ".ani",
}
DEFAULT_ALLOWED_ORIGINS = frozenset({
    "http://localhost:1420",
    "http://127.0.0.1:1420",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
})
CORS_METHODS = frozenset({"GET", "POST", "PUT", "DELETE", "OPTIONS"})
CORS_HEADERS = frozenset({
    "authorization", "content-type", "x-device-id", "x-sync-kind",
    "x-sync-record-id", "x-sync-revision", "x-upload-length", "x-upload-offset",
})


def is_loopback_host(host):
    value = str(host or "").strip().lower().strip("[]")
    if value == "localhost":
        return True
    try:
        return ipaddress.ip_address(value).is_loopback
    except ValueError:
        return False


def validate_bind_host(host):
    if not is_loopback_host(host):
        raise ValueError("El servidor HTTP solo puede enlazarse a loopback; usa un terminador TLS externo restringido a loopback para acceso de red")
    return host


def parse_allowed_origins(value=None):
    if value is None:
        value = os.environ.get("LUMINA_API_ALLOWED_ORIGINS", "")
    if isinstance(value, str):
        origins = [item.strip() for item in value.split(",") if item.strip()]
    else:
        origins = [str(item).strip() for item in value or [] if str(item).strip()]
    if not origins:
        return set(DEFAULT_ALLOWED_ORIGINS)
    result = set()
    for origin in origins:
        parsed = urlparse(origin)
        if origin == "*" or parsed.scheme not in ("http", "https") or not parsed.netloc or parsed.path or parsed.params or parsed.query or parsed.fragment:
            raise ValueError("Cada origen CORS debe ser una URL explícita sin comodines ni rutas")
        result.add(origin.rstrip("/"))
    return result


def now():
    return int(time.time())


def password_hash(password):
    if not isinstance(password, str) or len(password) < 10 or len(password) > 256:
        raise ValueError("La contraseña debe tener entre 10 y 256 caracteres")
    salt = secrets.token_bytes(16)
    value = hashlib.scrypt(password.encode(), salt=salt, n=2**15, r=8, p=1, maxmem=64 * 1024 * 1024)
    return f"scrypt$32768$8$1${salt.hex()}${value.hex()}"


def password_verify(password, encoded):
    try:
        scheme, n, r, p, salt, expected = encoded.split("$")
        if scheme != "scrypt":
            return False
        value = hashlib.scrypt(password.encode(), salt=bytes.fromhex(salt), n=int(n), r=int(r), p=int(p), maxmem=64 * 1024 * 1024)
        return hmac.compare_digest(value.hex(), expected)
    except (ValueError, TypeError):
        return False


def token_hash(token):
    return hashlib.sha256(token.encode()).hexdigest()


def valid_file(mime, header):
    return {
        "image/png": header.startswith(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg": header.startswith(b"\xff\xd8\xff"),
        "image/webp": header.startswith(b"RIFF") and header[8:12] == b"WEBP",
        "image/bmp": header.startswith(b"BM"),
        "image/x-icon": header.startswith(b"\x00\x00\x02\x00"),
        "application/octet-stream": header.startswith(b"RIFF") and header[8:12] == b"ACON",
    }.get(mime, False)


class Store:
    def __init__(self, path):
        self.path = path
        self.conn = sqlite3.connect(path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        self.conn.executescript("""
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS users(
          id TEXT PRIMARY KEY, email TEXT NOT NULL UNIQUE COLLATE NOCASE,
          password_hash TEXT NOT NULL, created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions(
          token_hash TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          expires_at INTEGER NOT NULL, revoked_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS devices(
          id TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          name TEXT NOT NULL, platform TEXT NOT NULL, last_sync_at INTEGER, revoked_at INTEGER,
          PRIMARY KEY(user_id, id)
        );
        CREATE TABLE IF NOT EXISTS sync_records(
          user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          record_id TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL,
          revision INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY(user_id, kind, record_id)
        );
        CREATE INDEX IF NOT EXISTS sync_records_user_revision ON sync_records(user_id, revision);
        CREATE TABLE IF NOT EXISTS files(
          id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          record_id TEXT NOT NULL, kind TEXT NOT NULL, original_name TEXT NOT NULL,
          mime TEXT NOT NULL, extension TEXT NOT NULL, size_bytes INTEGER NOT NULL,
          sha256 TEXT NOT NULL, storage_name TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS files_user_record ON files(user_id, record_id);
        CREATE UNIQUE INDEX IF NOT EXISTS files_user_hash ON files(user_id, sha256);
        CREATE TABLE IF NOT EXISTS file_records(
          file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
          user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          record_id TEXT NOT NULL, kind TEXT NOT NULL,
          PRIMARY KEY(file_id, record_id, kind)
        );
        CREATE TABLE IF NOT EXISTS upload_sessions(
          id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          device_id TEXT NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL,
          original_name TEXT NOT NULL, mime TEXT NOT NULL, expected_size INTEGER NOT NULL,
          expected_sha256 TEXT NOT NULL, temp_name TEXT NOT NULL UNIQUE, received_bytes INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
          final_file_id TEXT, final_storage_name TEXT,
          prepared_resolution_id TEXT, prepared_revision INTEGER
        );
        CREATE TABLE IF NOT EXISTS sync_resolutions(
          user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
          resolution_id TEXT NOT NULL,
          kind TEXT NOT NULL, record_id TEXT NOT NULL,
          expected_revision INTEGER NOT NULL, action TEXT NOT NULL,
          result_revision INTEGER NOT NULL, created_at INTEGER NOT NULL,
          PRIMARY KEY(user_id, resolution_id)
        );
        """)
        self._migrate_typed_devices()
        self._migrate_typed_sync_records()
        self._migrate_typed_file_records()
        self._migrate_upload_publication_intent()
        self.conn.commit()

    def _migrate_typed_devices(self):
        columns = self.conn.execute("PRAGMA table_info(devices)").fetchall()
        primary_key = [row[1] for row in sorted(columns, key=lambda row: row[5]) if row[5]]
        if primary_key == ["user_id", "id"]:
            return
        self.conn.execute("""
            CREATE TABLE devices_typed(
              id TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
              name TEXT NOT NULL, platform TEXT NOT NULL, last_sync_at INTEGER, revoked_at INTEGER,
              PRIMARY KEY(user_id, id)
            )
        """)
        self.conn.execute("""
            INSERT INTO devices_typed(id,user_id,name,platform,last_sync_at,revoked_at)
            SELECT id,user_id,name,platform,last_sync_at,revoked_at FROM devices
        """)
        self.conn.execute("DROP TABLE devices")
        self.conn.execute("ALTER TABLE devices_typed RENAME TO devices")

    def _migrate_typed_sync_records(self):
        columns = self.conn.execute("PRAGMA table_info(sync_records)").fetchall()
        primary_key = [row[1] for row in sorted(columns, key=lambda row: row[5]) if row[5]]
        if primary_key != ["user_id", "kind", "record_id"]:
            self.conn.execute("""
                CREATE TABLE sync_records_typed(
                  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                  record_id TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL,
                  revision INTEGER NOT NULL, updated_at INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0,
                  PRIMARY KEY(user_id, kind, record_id)
                )
            """)
            self.conn.execute("""
                INSERT INTO sync_records_typed(user_id,record_id,kind,payload,revision,updated_at,deleted)
                SELECT user_id,record_id,kind,payload,revision,updated_at,deleted FROM sync_records
            """)
            self.conn.execute("DROP TABLE sync_records")
            self.conn.execute("ALTER TABLE sync_records_typed RENAME TO sync_records")
        self.conn.execute("CREATE INDEX IF NOT EXISTS sync_records_user_revision ON sync_records(user_id, revision)")

    def _migrate_typed_file_records(self):
        columns = self.conn.execute("PRAGMA table_info(file_records)").fetchall()
        primary_key = [row[1] for row in sorted(columns, key=lambda row: row[5]) if row[5]]
        if primary_key == ["file_id", "record_id", "kind"]:
            return
        self.conn.execute("DROP TABLE IF EXISTS file_records_typed")
        self.conn.execute("""
            CREATE TABLE file_records_typed(
              file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
              user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
              record_id TEXT NOT NULL, kind TEXT NOT NULL,
              PRIMARY KEY(file_id, record_id, kind)
            )
        """)
        self.conn.execute("""
            INSERT INTO file_records_typed(file_id,user_id,record_id,kind)
            SELECT file_id,user_id,record_id,kind FROM file_records
        """)
        self.conn.execute("DROP TABLE file_records")
        self.conn.execute("ALTER TABLE file_records_typed RENAME TO file_records")

    def _migrate_upload_publication_intent(self):
        columns = {row[1] for row in self.conn.execute("PRAGMA table_info(upload_sessions)").fetchall()}
        if "final_file_id" not in columns:
            self.conn.execute("ALTER TABLE upload_sessions ADD COLUMN final_file_id TEXT")
        if "final_storage_name" not in columns:
            self.conn.execute("ALTER TABLE upload_sessions ADD COLUMN final_storage_name TEXT")
        if "prepared_resolution_id" not in columns:
            self.conn.execute("ALTER TABLE upload_sessions ADD COLUMN prepared_resolution_id TEXT")
        if "prepared_revision" not in columns:
            self.conn.execute("ALTER TABLE upload_sessions ADD COLUMN prepared_revision INTEGER")

    def recover_uploads(self, files_root):
        root = Path(files_root).resolve()
        rows = self.conn.execute(
            "SELECT * FROM upload_sessions WHERE status='publishing' OR (status='uploading' AND final_file_id IS NOT NULL) OR status='prepared'"
        ).fetchall()
        recovered = 0
        for upload in rows:
            final_id = upload["final_file_id"]
            relative = upload["final_storage_name"]
            extension = ALLOWED_FILES.get(upload["mime"])
            expected_relative = f"files/{upload['user_id']}/{final_id}{extension or ''}"
            final_path = (root / relative).resolve() if relative else None
            valid_path = (
                isinstance(final_id, str)
                and bool(re.fullmatch(r"[0-9a-f]{32}", final_id))
                and extension is not None
                and relative == expected_relative
                and final_path is not None
                and root in final_path.parents
            )
            source = (root / "uploads" / upload["temp_name"]).resolve()
            source_valid = source.parent == (root / "uploads").resolve()
            if upload["status"] == "prepared":
                if not source_valid or not source.is_file():
                    self.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload["id"]))
                    continue
                actual = hashlib.sha256(source.read_bytes()).hexdigest()
                header = source.read_bytes()[:512]
                if actual != upload["expected_sha256"] or source.stat().st_size != upload["expected_size"] or not valid_file(upload["mime"], header):
                    self.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload["id"]))
                continue
            if upload["status"] == "publishing" and upload["prepared_resolution_id"]:
                # A prepared conflict upload is not visible until the resolution transaction commits.
                # After an abrupt stop, restore the staged bytes and leave the operation retryable.
                confirmed = self.conn.execute(
                    "SELECT 1 FROM sync_resolutions WHERE user_id=? AND resolution_id=? AND kind=? AND record_id=?",
                    (upload["user_id"], upload["prepared_resolution_id"], upload["kind"], upload["record_id"]),
                ).fetchone()
                if confirmed and valid_path and final_path and final_path.is_file():
                    actual = hashlib.sha256(final_path.read_bytes()).hexdigest()
                    if final_path.stat().st_size == upload["expected_size"] and actual == upload["expected_sha256"]:
                        self.conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), upload["id"]))
                        recovered += 1
                        continue
                if source_valid and source.is_file():
                    self.conn.execute("UPDATE upload_sessions SET status='prepared',final_file_id=NULL,final_storage_name=NULL,updated_at=? WHERE id=?", (now(), upload["id"]))
                elif valid_path and final_path and final_path.is_file():
                    final_path.replace(source)
                    self.conn.execute("UPDATE upload_sessions SET status='prepared',final_file_id=NULL,final_storage_name=NULL,updated_at=? WHERE id=?", (now(), upload["id"]))
                else:
                    self.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload["id"]))
                continue
            existing = self.conn.execute(
                "SELECT * FROM files WHERE id=? AND user_id=?",
                (final_id, upload["user_id"]),
            ).fetchone() if valid_path else None
            if existing:
                relation = self.conn.execute(
                    "SELECT 1 FROM file_records WHERE file_id=? AND user_id=? AND record_id=? AND kind=?",
                    (final_id, upload["user_id"], upload["record_id"], upload["kind"]),
                ).fetchone()
                if relation and existing["sha256"] == upload["expected_sha256"]:
                    self.conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), upload["id"]))
                    recovered += 1
                continue
            if valid_path and final_path.is_file():
                actual = hashlib.sha256(final_path.read_bytes()).hexdigest()
                header = final_path.read_bytes()[:512]
                if actual == upload["expected_sha256"] and final_path.stat().st_size == upload["expected_size"] and valid_file(upload["mime"], header):
                    existing_hash = self.conn.execute(
                        "SELECT id FROM files WHERE user_id=? AND kind=? AND sha256=?",
                        (upload["user_id"], upload["kind"], upload["expected_sha256"]),
                    ).fetchone()
                    relation_file_id = existing_hash["id"] if existing_hash else final_id
                    if not existing_hash:
                        self.conn.execute("INSERT INTO files VALUES(?,?,?,?,?,?,?,?,?,?,?)", (final_id, upload["user_id"], upload["record_id"], upload["kind"], upload["original_name"], upload["mime"], extension, upload["expected_size"], upload["expected_sha256"], relative, now()))
                    self.conn.execute("INSERT OR IGNORE INTO file_records VALUES(?,?,?,?)", (relation_file_id, upload["user_id"], upload["record_id"], upload["kind"]))
                    self.conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), upload["id"]))
                    if existing_hash or source_valid:
                        final_path.unlink(missing_ok=True) if existing_hash else source.unlink(missing_ok=True)
                    recovered += 1
                    continue
                final_path.unlink(missing_ok=True)
            if source_valid and source.is_file():
                self.conn.execute("UPDATE upload_sessions SET status='uploading',final_file_id=NULL,final_storage_name=NULL,updated_at=? WHERE id=?", (now(), upload["id"]))
            else:
                self.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload["id"]))
        self.conn.commit()
        return recovered

    def cleanup_expired_uploads(self, files_root, now_value=None, ttl=24 * 60 * 60):
        cutoff = (now() if now_value is None else now_value) - ttl
        uploads_root = Path(files_root).resolve() / "uploads"
        removed = 0
        rows = self.conn.execute(
            "SELECT id,temp_name FROM upload_sessions WHERE status IN ('uploading','prepared') AND final_file_id IS NULL AND updated_at < ?",
            (cutoff,),
        ).fetchall()
        for row in rows:
            temp_name = row["temp_name"]
            if (
                not re.fullmatch(r"[0-9a-f]{32}\.part", temp_name or "")
                or not temp_name.startswith(row["id"])
            ):
                continue
            candidate = (uploads_root / temp_name).resolve()
            if uploads_root not in candidate.parents or candidate.parent != uploads_root:
                continue
            self.conn.execute("DELETE FROM upload_sessions WHERE id=? AND status='uploading'", (row["id"],))
            candidate.unlink(missing_ok=True)
            removed += 1
        self.conn.commit()
        return removed

    def close(self):
        self.conn.close()

    def user(self, email):
        return self.conn.execute("SELECT * FROM users WHERE email = ?", (email,)).fetchone()

    def session_user(self, token):
        row = self.conn.execute(
            "SELECT u.* FROM sessions s JOIN users u ON u.id=s.user_id "
            "WHERE s.token_hash=? AND s.revoked_at IS NULL AND s.expires_at>?",
            (token_hash(token), now()),
        ).fetchone()
        return row

    def device_for(self, device_id, user_id):
        return self.conn.execute(
            "SELECT * FROM devices WHERE id=? AND user_id=? AND revoked_at IS NULL",
            (device_id, user_id),
        ).fetchone()


class Api(BaseHTTPRequestHandler):
    server_version = "LuminaSync/1.0"

    def request_origin(self):
        origin = self.headers.get("Origin")
        return origin.rstrip("/") if origin else None

    def origin_allowed(self):
        origin = self.request_origin()
        return origin is None or origin in getattr(self.server, "allowed_origins", DEFAULT_ALLOWED_ORIGINS)

    def cors_headers(self, origin):
        if origin and origin in getattr(self.server, "allowed_origins", DEFAULT_ALLOWED_ORIGINS):
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")

    def reject_disallowed_origin(self):
        if self.origin_allowed():
            return False
        self.json(403, {"error": "Origen no permitido"})
        return True

    def json(self, status, value):
        raw = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(raw)))
        self.cors_headers(self.request_origin())
        self.end_headers()
        self.wfile.write(raw)

    def body(self):
        length = int(self.headers.get("Content-Length", "0"))
        if length > MAX_BODY:
            raise ValueError("La solicitud es demasiado grande")
        raw = self.rfile.read(length)
        return json.loads(raw or b"{}")

    def auth(self):
        value = self.headers.get("Authorization", "")
        if not value.startswith("Bearer "):
            return None
        return self.server.store.session_user(value[7:])

    def do_OPTIONS(self):
        origin = self.request_origin()
        if not origin or not self.origin_allowed():
            return self.json(403, {"error": "Origen CORS no permitido"})
        requested_method = self.headers.get("Access-Control-Request-Method", "GET").upper()
        requested_headers = {
            header.strip().lower()
            for header in self.headers.get("Access-Control-Request-Headers", "").split(",")
            if header.strip()
        }
        if requested_method not in CORS_METHODS or not requested_headers.issubset(CORS_HEADERS):
            return self.json(403, {"error": "Solicitud CORS no permitida"})
        self.send_response(204)
        self.cors_headers(origin)
        self.send_header("Access-Control-Allow-Headers", ", ".join(sorted(CORS_HEADERS)))
        self.send_header("Access-Control-Allow-Methods", ", ".join(sorted(CORS_METHODS)))
        self.send_header("Access-Control-Max-Age", "600")
        self.end_headers()

    def do_POST(self):
        if self.reject_disallowed_origin():
            return
        try:
            path = urlparse(self.path).path
            if path in ("/v1/auth/register", "/v1/auth/login") and not self.server.allow_auth_attempt(self.client_address[0]):
                return self.json(429, {"error": "Demasiados intentos; inténtalo más tarde"})
            if path.startswith("/v1/files/") and not getattr(self.server, "allow_transfer_attempt", lambda address: True)(self.client_address[0]):
                return self.json(429, {"error": "Demasiadas transferencias; inténtalo más tarde"})
            data = self.body()
            if path == "/v1/auth/register":
                return self.register(data)
            if path == "/v1/auth/login":
                return self.login(data)
            if path == "/v1/auth/logout":
                return self.logout()
            if path == "/v1/devices":
                return self.device(data)
            if path == "/v1/sync":
                return self.sync(data)
            if path == "/v1/sync/resolve":
                return self.resolve_sync(data)
            if path == "/v1/files/upload/init":
                with self.upload_guard():
                    self.server.store.cleanup_expired_uploads(self.server.files_root)
                    return self.upload_init(data)
            if path.startswith("/v1/files/upload/") and path.endswith("/complete"):
                with self.upload_guard():
                    return self.upload_complete(unquote(path[len("/v1/files/upload/"):-len("/complete")]))
            self.json(404, {"error": "Ruta no encontrada"})
        except ValueError as error:
            self.json(400, {"error": str(error)})
        except Exception:
            self.json(500, {"error": "Error interno del servidor"})

    def do_GET(self):
        if self.reject_disallowed_origin():
            return
        path = urlparse(self.path).path
        if path.startswith("/v1/files/") and not getattr(self.server, "allow_transfer_attempt", lambda address: True)(self.client_address[0]):
            return self.json(429, {"error": "Demasiadas transferencias; inténtalo más tarde"})
        user = self.auth()
        if path == "/v1/auth/session":
            if not user:
                return self.json(401, {"authenticated": False})
            return self.json(200, {"authenticated": True, "user": {"id": user["id"], "email": user["email"]}})
        if path == "/v1/devices":
            if not user:
                return self.json(401, {"error": "Sesión no válida"})
            rows = self.server.store.conn.execute("SELECT id,name,platform,last_sync_at,revoked_at FROM devices WHERE user_id=?", (user["id"],)).fetchall()
            return self.json(200, {"devices": [dict(row) for row in rows]})
        if path.startswith("/v1/files/upload/"):
            with self.upload_guard():
                return self.upload_status(unquote(path[len("/v1/files/upload/"):]))
        if path.startswith("/v1/files/"):
            return self.download(unquote(path[len("/v1/files/"):]))
        self.json(404, {"error": "Ruta no encontrada"})

    def do_PUT(self):
        if self.reject_disallowed_origin():
            return
        path = urlparse(self.path).path
        if not path.startswith("/v1/files/upload/") or path.endswith("/complete"):
            return self.json(404, {"error": "Ruta no encontrada"})
        if not getattr(self.server, "allow_transfer_attempt", lambda address: True)(self.client_address[0]):
            return self.json(429, {"error": "Demasiadas transferencias; inténtalo más tarde"})
        try:
            with self.upload_guard():
                self.upload_chunk(unquote(path[len("/v1/files/upload/"):]))
        except ValueError as error:
            self.json(400, {"error": str(error)})
        except Exception:
            self.json(500, {"error": "No se pudo recibir la transferencia"})

    def upload_guard(self):
        # Some tests construct Api manually instead of going through run().
        if not hasattr(self.server, "upload_lock"):
            self.server.upload_lock = threading.RLock()
        return self.server.upload_lock

    def do_DELETE(self):
        if self.reject_disallowed_origin():
            return
        path = unquote(urlparse(self.path).path)
        user = self.auth()
        if not user:
            return self.json(401, {"error": "Sesión no válida"})
        prefix = "/v1/devices/"
        if path.startswith(prefix):
            device_id = path[len(prefix):]
            self.server.store.conn.execute("UPDATE devices SET revoked_at=? WHERE id=? AND user_id=?", (now(), device_id, user["id"]))
            self.server.store.conn.commit()
            return self.json(200, {"revoked": True})
        self.json(404, {"error": "Ruta no encontrada"})

    def register(self, data):
        email = str(data.get("email", "")).strip().lower()
        if "@" not in email or len(email) > 320:
            raise ValueError("Correo electrónico no válido")
        password = data.get("password")
        encoded = password_hash(password)
        user_id = secrets.token_hex(16)
        try:
            self.server.store.conn.execute("INSERT INTO users VALUES(?,?,?,?)", (user_id, email, encoded, now()))
            self.server.store.conn.commit()
        except sqlite3.IntegrityError:
            return self.json(409, {"error": "No se pudo crear la cuenta con esos datos"})
        return self.issue(user_id, email)

    def login(self, data):
        email = str(data.get("email", "")).strip().lower()
        user = self.server.store.user(email)
        if not user or not password_verify(str(data.get("password", "")), user["password_hash"]):
            return self.json(401, {"error": "Credenciales no válidas"})
        return self.issue(user["id"], user["email"])

    def issue(self, user_id, email):
        token = secrets.token_urlsafe(32)
        self.server.store.conn.execute("INSERT INTO sessions VALUES(?,?,?,NULL)", (token_hash(token), user_id, now() + TOKEN_TTL))
        self.server.store.conn.commit()
        return self.json(200, {"token": token, "expires_at": now() + TOKEN_TTL, "user": {"id": user_id, "email": email}})

    def logout(self):
        header = self.headers.get("Authorization", "")
        if header.startswith("Bearer "):
            self.server.store.conn.execute("UPDATE sessions SET revoked_at=? WHERE token_hash=?", (now(), token_hash(header[7:])))
            self.server.store.conn.commit()
        return self.json(200, {"logged_out": True})

    def device(self, data):
        user = self.auth()
        if not user:
            return self.json(401, {"error": "Sesión no válida"})
        device_id = str(data.get("id", "")).strip()
        name = str(data.get("name", "")).strip()
        platform = str(data.get("platform", "")).strip().lower()
        if not device_id or len(device_id) > 128 or not name or len(name) > 120 or platform not in ("windows", "android", "linux", "macos"):
            raise ValueError("Datos de dispositivo no válidos")
        self.server.store.conn.execute(
            "INSERT INTO devices(id,user_id,name,platform,last_sync_at,revoked_at) VALUES(?,?,?,?,NULL,NULL) "
            "ON CONFLICT(user_id,id) DO UPDATE SET name=excluded.name, platform=excluded.platform, revoked_at=NULL",
            (device_id, user["id"], name, platform))
        self.server.store.conn.commit()
        return self.json(200, {"device_id": device_id})

    def sync(self, data):
        with self.server.sync_lock:
            return self._sync_locked(data)

    def resolve_prepared_local(self, user, device_id, kind, record_id, expected_revision, resolution_id, local_payload, prepared_upload_id):
        if kind != "wallpaper" or not prepared_upload_id:
            raise ValueError("La resolución binaria requiere una carga preparada")
        conn = self.server.store.conn
        previous = conn.execute("SELECT kind,record_id,expected_revision,action,result_revision FROM sync_resolutions WHERE user_id=? AND resolution_id=?", (user["id"], resolution_id)).fetchone()
        if previous:
            if (previous["kind"], previous["record_id"], previous["expected_revision"], previous["action"]) != (kind, record_id, expected_revision, "keep_local"):
                raise ValueError("El identificador de resolución ya fue usado para otra operación")
            return self.json(200, {"resolved": True, "idempotent": True, "kind": kind, "record_id": record_id, "revision": previous["result_revision"]})
        current = conn.execute("SELECT * FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
        if not current or current["revision"] != expected_revision or current["deleted"]:
            return self.json(409, {"error": "La revisión remota cambió; vuelve a sincronizar antes de resolver"})
        upload = conn.execute("SELECT * FROM upload_sessions WHERE id=? AND user_id=? AND device_id=? AND kind=? AND record_id=? AND status='prepared'", (prepared_upload_id, user["id"], device_id, kind, record_id)).fetchone()
        if not upload or upload["prepared_resolution_id"] != resolution_id or upload["prepared_revision"] != expected_revision:
            return self.json(409, {"error": "La carga preparada no corresponde a este conflicto"})
        if not isinstance(local_payload, dict):
            raise ValueError("La resolución local requiere metadatos válidos")
        if str(local_payload.get("hash", "")).lower() != upload["expected_sha256"].lower() or int(local_payload.get("size_bytes", -1)) != upload["expected_size"]:
            return self.json(409, {"error": "Los metadatos locales no coinciden con la carga preparada"})
        root = self.server.files_root.resolve(); source = (root / "uploads" / upload["temp_name"]).resolve()
        if source.parent != (root / "uploads").resolve() or not source.is_file():
            return self.json(409, {"error": "La carga preparada no está disponible"})
        content = source.read_bytes(); first = content[:512]
        if len(content) != upload["expected_size"] or hashlib.sha256(content).hexdigest() != upload["expected_sha256"] or not valid_file(upload["mime"], first):
            return self.json(409, {"error": "La carga preparada no coincide con su integridad declarada"})
        extension = ALLOWED_FILES[upload["mime"]]
        existing = conn.execute("SELECT * FROM files WHERE user_id=? AND kind=? AND sha256=?", (user["id"], kind, upload["expected_sha256"])).fetchone()
        if existing:
            existing_path = (root / existing["storage_name"]).resolve()
            if existing_path.parent == (root / "files" / user["id"]).resolve() and existing_path.is_file() and existing_path.stat().st_size == upload["expected_size"]:
                result_revision = expected_revision + 1
                conn.execute("BEGIN")
                try:
                    latest = conn.execute("SELECT revision,deleted FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
                    if not latest or latest["revision"] != expected_revision or latest["deleted"]:
                        raise RuntimeError("La revisión remota cambió durante la resolución")
                    conn.execute("INSERT OR IGNORE INTO file_records VALUES(?,?,?,?)", (existing["id"], user["id"], record_id, kind))
                    conn.execute("UPDATE sync_records SET payload=?,revision=?,updated_at=?,deleted=0 WHERE user_id=? AND kind=? AND record_id=? AND revision=?", (json.dumps(local_payload, ensure_ascii=False), result_revision, now(), user["id"], kind, record_id, expected_revision))
                    conn.execute("INSERT INTO sync_resolutions VALUES(?,?,?,?,?,?,?,?)", (user["id"], resolution_id, kind, record_id, expected_revision, "keep_local", result_revision, now()))
                    conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), prepared_upload_id)); conn.commit()
                except Exception:
                    conn.rollback(); raise
                source.unlink(missing_ok=True)
                return self.json(200, {"resolved": True, "idempotent": False, "kind": kind, "record_id": record_id, "revision": result_revision, "deleted": False, "payload": local_payload, "file": dict(existing)})
        file_id = secrets.token_hex(16); relative = f"files/{user['id']}/{file_id}{extension}"; target = (root / relative).resolve()
        conn.execute("UPDATE upload_sessions SET status='publishing',final_file_id=?,final_storage_name=?,updated_at=? WHERE id=? AND status='prepared'", (file_id, relative, now(), prepared_upload_id)); conn.commit()
        moved = False
        try:
            target.parent.mkdir(parents=True, exist_ok=True); source.replace(target); moved = True
            conn.execute("BEGIN")
            if getattr(self.server, "before_resolution_persist", None):
                self.server.before_resolution_persist()
            latest = conn.execute("SELECT revision,deleted FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
            if not latest or latest["revision"] != expected_revision or latest["deleted"]:
                raise RuntimeError("La revisión remota cambió durante la resolución")
            result_revision = expected_revision + 1
            conn.execute("INSERT INTO files VALUES(?,?,?,?,?,?,?,?,?,?,?)", (file_id, user["id"], record_id, kind, upload["original_name"], upload["mime"], extension, upload["expected_size"], upload["expected_sha256"], relative, now()))
            conn.execute("INSERT INTO file_records VALUES(?,?,?,?)", (file_id, user["id"], record_id, kind))
            conn.execute("UPDATE sync_records SET payload=?,revision=?,updated_at=?,deleted=0 WHERE user_id=? AND kind=? AND record_id=? AND revision=?", (json.dumps(local_payload, ensure_ascii=False), result_revision, now(), user["id"], kind, record_id, expected_revision))
            conn.execute("INSERT INTO sync_resolutions VALUES(?,?,?,?,?,?,?,?)", (user["id"], resolution_id, kind, record_id, expected_revision, "keep_local", result_revision, now()))
            conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), prepared_upload_id)); conn.commit()
        except Exception:
            conn.rollback()
            try:
                if moved and target.is_file() and not source.exists(): target.replace(source)
                conn.execute("UPDATE upload_sessions SET status='prepared',final_file_id=NULL,final_storage_name=NULL,updated_at=? WHERE id=?", (now(), prepared_upload_id)); conn.commit()
            except Exception:
                conn.rollback()
            raise
        return self.json(200, {"resolved": True, "idempotent": False, "kind": kind, "record_id": record_id, "revision": result_revision, "deleted": False, "payload": local_payload, "file": {"id": file_id, "record_id": record_id, "kind": kind, "original_name": upload["original_name"], "mime": upload["mime"], "extension": extension, "size_bytes": upload["expected_size"], "sha256": upload["expected_sha256"], "storage_name": relative}})

    def resolve_sync(self, data):
        with self.server.sync_lock:
            user = self.auth()
            if not user:
                return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
            device_id = str(data.get("device_id", "")).strip()
            if not self.server.store.device_for(device_id, user["id"]):
                return self.json(403, {"error": "Dispositivo no vinculado o revocado"})
            kind = str(data.get("kind", "")).strip()
            record_id = str(data.get("record_id", "")).strip()
            action = str(data.get("action", "")).strip()
            resolution_id = str(data.get("resolution_id", "")).strip()
            if kind not in ("wallpaper", "cursor", "playlist", "playlist_item", "setting") or not record_id or not resolution_id or action not in ("keep_remote", "keep_local"):
                raise ValueError("Datos de resolución no válidos")
            try:
                expected_revision = int(data.get("expected_revision"))
            except (TypeError, ValueError):
                raise ValueError("La revisión esperada no es válida")
            if expected_revision < 1 or len(resolution_id) > 128:
                raise ValueError("Datos de resolución no válidos")
            local_payload = data.get("local_payload")
            if action == "keep_local" and not isinstance(local_payload, dict):
                raise ValueError("La resolución local requiere metadatos válidos")
            if action == "keep_local" and kind == "wallpaper" and not bool(data.get("deleted", False)):
                return self.resolve_prepared_local(user, device_id, kind, record_id, expected_revision, resolution_id, local_payload, str(data.get("prepared_upload_id", "")).strip())
            with self.server.store.conn:
                previous = self.server.store.conn.execute(
                    "SELECT kind,record_id,expected_revision,action,result_revision FROM sync_resolutions WHERE user_id=? AND resolution_id=?",
                    (user["id"], resolution_id),
                ).fetchone()
                if previous:
                    if (previous["kind"], previous["record_id"], previous["expected_revision"], previous["action"]) != (kind, record_id, expected_revision, action):
                        raise ValueError("El identificador de resolución ya fue usado para otra operación")
                    return self.json(200, {"resolved": True, "idempotent": True, "kind": kind, "record_id": record_id, "revision": previous["result_revision"]})
                current = self.server.store.conn.execute(
                    "SELECT record_id,kind,payload,revision,updated_at,deleted FROM sync_records WHERE user_id=? AND kind=? AND record_id=?",
                    (user["id"], kind, record_id),
                ).fetchone()
                if not current or current["revision"] != expected_revision:
                    return self.json(409, {"error": "La revisión remota cambió; vuelve a sincronizar antes de resolver"})
                requested_deleted = bool(data.get("deleted", False))
                if action == "keep_local" and current["deleted"] and not requested_deleted:
                    return self.json(409, {"error": "Remote record is deleted; confirm the local tombstone explicitly"})
                remote_file = None
                if action == "keep_remote" and kind in ("wallpaper", "cursor") and not current["deleted"]:
                    remote_file = self.server.store.conn.execute(
                        "SELECT id,record_id,kind,original_name,mime,extension,size_bytes,sha256,storage_name FROM files WHERE user_id=? AND kind=? AND record_id=? LIMIT 1",
                        (user["id"], kind, record_id),
                    ).fetchone()
                    if not remote_file:
                        return self.json(409, {"error": "Remote file unavailable"})
                    target = (self.server.files_root / remote_file["storage_name"]).resolve()
                    root = self.server.files_root.resolve()
                    if root not in target.parents or not target.is_file() or target.stat().st_size != remote_file["size_bytes"]:
                        return self.json(409, {"error": "Remote file is invalid"})
                    digest = hashlib.sha256()
                    first = b""
                    with target.open("rb") as stream:
                        while chunk := stream.read(CHUNK_SIZE):
                            if not first:
                                first = chunk[:512]
                            digest.update(chunk)
                    if digest.hexdigest().lower() != remote_file["sha256"].lower() or not valid_file(remote_file["mime"], first):
                        return self.json(409, {"error": "Remote file does not match its metadata"})
                    try:
                        remote_payload = json.loads(current["payload"])
                    except (TypeError, ValueError):
                        remote_payload = {}
                    try:
                        payload_size = int(remote_payload.get("size_bytes", -1))
                    except (TypeError, ValueError):
                        payload_size = -1
                    if str(remote_payload.get("hash", "")).lower() != remote_file["sha256"].lower() or payload_size != remote_file["size_bytes"]:
                        return self.json(409, {"error": "Remote file does not match the revision"})
                result_revision = current["revision"]
                if action == "keep_local":
                    result_revision += 1
                    self.server.store.conn.execute(
                        "UPDATE sync_records SET payload=?,revision=?,updated_at=?,deleted=? WHERE user_id=? AND kind=? AND record_id=? AND revision=?",
                        (json.dumps(local_payload, ensure_ascii=False), result_revision, now(), int(requested_deleted), user["id"], kind, record_id, expected_revision),
                    )
                    self.server.store.conn.execute("UPDATE devices SET last_sync_at=? WHERE id=? AND user_id=?", (now(), device_id, user["id"]))
                self.server.store.conn.execute(
                    "INSERT INTO sync_resolutions VALUES(?,?,?,?,?,?,?,?)",
                    (user["id"], resolution_id, kind, record_id, expected_revision, action, result_revision, now()),
                )
                payload = json.loads(current["payload"]) if action == "keep_remote" else local_payload
                deleted = bool(current["deleted"]) if action == "keep_remote" else requested_deleted
            response = {"resolved": True, "idempotent": False, "kind": kind, "record_id": record_id, "revision": result_revision, "deleted": deleted, "payload": payload}
            if remote_file:
                response["file"] = dict(remote_file)
            return self.json(200, response)

    def _sync_locked(self, data):
        with self.server.store.conn:
            return self._sync_locked_unmanaged(data)

    def _sync_locked_unmanaged(self, data):
        user = self.auth()
        if not user:
            return self.json(401, {"error": "Sesión no válida"})
        device_id = str(data.get("device_id", ""))
        device = self.server.store.device_for(device_id, user["id"])
        if not device:
            return self.json(403, {"error": "Dispositivo no vinculado o revocado"})
        changes = data.get("changes", [])
        if not isinstance(changes, list) or len(changes) > 500:
            raise ValueError("Cambios de sincronización no válidos")
        conflicts = []
        for change in changes:
            record_id, kind = str(change.get("id", "")), str(change.get("kind", ""))
            if not record_id or kind not in ("wallpaper", "playlist", "playlist_item", "setting", "cursor"):
                raise ValueError("Registro de sincronización no válido")
            current = self.server.store.conn.execute("SELECT * FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
            base = int(change.get("base_revision", 0))
            if current and current["revision"] != base:
                conflicts.append({"id": record_id, "kind": kind, "server_revision": current["revision"], "server_payload": json.loads(current["payload"])})
                continue
            revision = (current["revision"] if current else 0) + 1
            self.server.store.conn.execute(
                "INSERT INTO sync_records VALUES(?,?,?,?,?,?,?) ON CONFLICT(user_id,kind,record_id) DO UPDATE SET payload=excluded.payload,revision=excluded.revision,updated_at=excluded.updated_at,deleted=excluded.deleted",
                (user["id"], record_id, kind, json.dumps(change.get("payload", {}), ensure_ascii=False), revision, now(), int(bool(change.get("deleted", False)))))
        self.server.store.conn.execute("UPDATE devices SET last_sync_at=? WHERE id=? AND user_id=?", (now(), device_id, user["id"]))
        self.server.store.conn.commit()
        rows = self.server.store.conn.execute("SELECT record_id,kind,payload,revision,updated_at,deleted FROM sync_records WHERE user_id=?", (user["id"],)).fetchall()
        files = self.server.store.conn.execute("SELECT id,record_id,kind,original_name,mime,extension,size_bytes,sha256 FROM files WHERE user_id=?", (user["id"],)).fetchall()
        return self.json(200, {"records": [{**dict(row), "payload": json.loads(row["payload"])} for row in rows], "files": [dict(row) for row in files], "conflicts": conflicts})

    def transfer_context(self, upload_id):
        user = self.auth()
        if not user:
            return None, None
        upload = self.server.store.conn.execute(
            "SELECT * FROM upload_sessions WHERE id=? AND user_id=? AND status='uploading'",
            (upload_id, user["id"]),
        ).fetchone()
        if not upload or not self.server.store.device_for(upload["device_id"], user["id"]):
            return user, None
        return user, upload

    def upload_init(self, data):
        user = self.auth()
        if not user:
            return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
        device_id = str(data.get("device_id", "")); record_id = str(data.get("record_id", "")); kind = str(data.get("kind", ""))
        raw_name = str(data.get("name", "")).strip(); name = os.path.basename(raw_name); mime = str(data.get("mime", "")).lower()
        size = data.get("size"); sha256 = str(data.get("sha256", "")).lower()
        if not self.server.store.device_for(device_id, user["id"]):
            return self.json(403, {"error": "Dispositivo no vinculado o revocado"})
        if kind not in ("wallpaper", "cursor") or not record_id or not name or name in (".", "..") or len(name) > 255 or raw_name != name or any(ord(char) < 32 for char in name):
            raise ValueError("Identidad del archivo no vÃ¡lida")
        if mime not in ALLOWED_FILES or not isinstance(size, int) or size <= 0 or size > MAX_FILE_SIZE or len(sha256) != 64:
            raise ValueError("Tipo, tamaÃ±o o hash de archivo no vÃ¡lido")
        try:
            int(sha256, 16)
        except ValueError:
            raise ValueError("Hash de archivo no vÃ¡lido")
        prepared = bool(data.get("defer_resolution", False))
        prepared_resolution_id = str(data.get("resolution_id", "")).strip() if prepared else None
        prepared_revision = data.get("expected_revision") if prepared else None
        if prepared:
            if not prepared_resolution_id or len(prepared_resolution_id) > 128:
                raise ValueError("La carga preparada requiere un identificador de resolución")
            try:
                prepared_revision = int(prepared_revision)
            except (TypeError, ValueError):
                raise ValueError("La revisión preparada no es válida")
            if prepared_revision < 1:
                raise ValueError("La revisión preparada no es válida")
        record = self.server.store.conn.execute("SELECT kind FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
        if not record or record["kind"] != kind:
            return self.json(409, {"error": "Sincroniza primero el registro de metadatos"})
        if prepared:
            previous = self.server.store.conn.execute("SELECT * FROM upload_sessions WHERE user_id=? AND device_id=? AND kind=? AND record_id=? AND prepared_resolution_id=?", (user["id"], device_id, kind, record_id, prepared_resolution_id)).fetchone()
            if previous:
                if previous["expected_sha256"] != sha256 or previous["expected_size"] != size or previous["prepared_revision"] != prepared_revision:
                    raise ValueError("La carga preparada ya fue usada con contenido incompatible")
                return self.json(200, {"upload_id": previous["id"], "offset": previous["received_bytes"], "chunk_size": CHUNK_SIZE, "prepared": previous["status"] in ("prepared", "complete"), "status": previous["status"]})
            current = self.server.store.conn.execute("SELECT revision,deleted FROM sync_records WHERE user_id=? AND kind=? AND record_id=?", (user["id"], kind, record_id)).fetchone()
            if not current or current["revision"] != prepared_revision or current["deleted"]:
                return self.json(409, {"error": "La revisión remota ya no coincide con la resolución"})
        existing = None if prepared else self.server.store.conn.execute("SELECT id,size_bytes,sha256 FROM files WHERE user_id=? AND kind=? AND sha256=?", (user["id"], kind, sha256)).fetchone()
        if existing:
            self.server.store.conn.execute("INSERT OR IGNORE INTO file_records VALUES(?,?,?,?)", (existing["id"], user["id"], record_id, kind)); self.server.store.conn.commit()
            return self.json(200, {"file_id": existing["id"], "already_exists": True, "size": existing["size_bytes"], "sha256": existing["sha256"]})
        upload_id = secrets.token_hex(16); temp_name = f"{upload_id}.part"; stamp = now()
        self.server.store.conn.execute(
            "INSERT INTO upload_sessions(user_id,device_id,record_id,kind,original_name,mime,expected_size,expected_sha256,temp_name,received_bytes,status,created_at,updated_at,final_file_id,final_storage_name,prepared_resolution_id,prepared_revision,id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (user["id"], device_id, record_id, kind, name, mime, size, sha256, temp_name, 0, "uploading", stamp, stamp, None, None, prepared_resolution_id, prepared_revision, upload_id),
        )
        self.server.store.conn.commit()
        (self.server.files_root / "uploads" / temp_name).touch()
        return self.json(201, {"upload_id": upload_id, "offset": 0, "chunk_size": CHUNK_SIZE})

    def upload_chunk(self, upload_id):
        user, upload = self.transfer_context(upload_id)
        if not user:
            return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
        if not upload:
            return self.json(403, {"error": "Transferencia no autorizada o expirada"})
        offset = int(self.headers.get("X-Upload-Offset", "-1")); length = int(self.headers.get("X-Upload-Length", self.headers.get("Content-Length", "-1")))
        if offset != upload["received_bytes"] or length < 0 or length > CHUNK_SIZE or offset + length > upload["expected_size"]:
            raise ValueError("Offset o tamaÃ±o de bloque no vÃ¡lido")
        target = self.server.files_root / "uploads" / upload["temp_name"]
        with target.open("ab") as stream:
            remaining = length
            while remaining:
                chunk = self.rfile.read(min(CHUNK_SIZE, remaining))
                if not chunk:
                    raise ValueError("Transferencia incompleta")
                stream.write(chunk); remaining -= len(chunk)
        received = offset + length; stamp = now()
        self.server.store.conn.execute("UPDATE upload_sessions SET received_bytes=?,updated_at=? WHERE id=?", (received, stamp, upload_id)); self.server.store.conn.commit()
        return self.json(200, {"upload_id": upload_id, "offset": received, "complete": received == upload["expected_size"]})

    def upload_status(self, upload_id):
        user, upload = self.transfer_context(upload_id)
        if not user:
            return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
        if not upload:
            return self.json(404, {"error": "Transferencia no encontrada"})
        return self.json(200, {"upload_id": upload_id, "offset": upload["received_bytes"], "size": upload["expected_size"], "sha256": upload["expected_sha256"]})

    def upload_complete(self, upload_id):
        user = self.auth()
        if not user:
            return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
        upload = self.server.store.conn.execute("SELECT * FROM upload_sessions WHERE id=? AND user_id=?", (upload_id, user["id"])).fetchone()
        if upload and upload["status"] == "complete":
            existing = self.server.store.conn.execute("SELECT id,size_bytes,sha256 FROM files WHERE user_id=? AND kind=? AND sha256=?", (user["id"], upload["kind"], upload["expected_sha256"])).fetchone()
            if existing:
                return self.json(200, {"file_id": existing["id"], "already_exists": True, "size": existing["size_bytes"], "sha256": existing["sha256"]})
        if upload and upload["status"] == "prepared":
            return self.json(200, {"upload_id": upload_id, "prepared": True, "status": "prepared", "size": upload["expected_size"], "sha256": upload["expected_sha256"]})
        upload = upload if upload and self.server.store.device_for(upload["device_id"], user["id"]) and upload["status"] == "uploading" else None
        if not upload:
            return self.json(403, {"error": "Transferencia no autorizada o expirada"})
        source = self.server.files_root / "uploads" / upload["temp_name"]
        if upload["received_bytes"] != upload["expected_size"] or not source.is_file():
            return self.json(409, {"error": "La transferencia aún no está completa"})
        actual = hashlib.sha256(); first = b""
        with source.open("rb") as stream:
            while chunk := stream.read(CHUNK_SIZE):
                if not first: first = chunk[:512]
                actual.update(chunk)
        if actual.hexdigest() != upload["expected_sha256"] or not valid_file(upload["mime"], first):
            source.unlink(missing_ok=True)
            self.server.store.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload_id)); self.server.store.conn.commit()
            return self.json(422, {"error": "El contenido no coincide con el tipo o hash declarado"})
        if upload["prepared_resolution_id"]:
            self.server.store.conn.execute("UPDATE upload_sessions SET status='prepared',updated_at=? WHERE id=? AND status='uploading'", (now(), upload_id))
            self.server.store.conn.commit()
            return self.json(201, {"upload_id": upload_id, "prepared": True, "status": "prepared", "size": upload["expected_size"], "sha256": upload["expected_sha256"], "resolution_id": upload["prepared_resolution_id"]})
        file_id = secrets.token_hex(16); extension = ALLOWED_FILES[upload["mime"]]; relative = f"files/{user['id']}/{file_id}{extension}"; target = self.server.files_root / relative
        self.server.store.conn.execute("UPDATE upload_sessions SET status='publishing',final_file_id=?,final_storage_name=?,updated_at=? WHERE id=? AND status='uploading'", (file_id, relative, now(), upload_id)); self.server.store.conn.commit()
        target.parent.mkdir(parents=True, exist_ok=True); source.replace(target)
        try:
            self.server.store.conn.execute("BEGIN")
            failure_hook = getattr(self.server, "before_upload_persist", None)
            if failure_hook:
                failure_hook()
            self.server.store.conn.execute("INSERT INTO files VALUES(?,?,?,?,?,?,?,?,?,?,?)", (file_id, user["id"], upload["record_id"], upload["kind"], upload["original_name"], upload["mime"], extension, upload["expected_size"], upload["expected_sha256"], relative, now()))
            self.server.store.conn.execute("INSERT INTO file_records VALUES(?,?,?,?)", (file_id, user["id"], upload["record_id"], upload["kind"]))
            self.server.store.conn.execute("UPDATE upload_sessions SET status='complete',updated_at=? WHERE id=?", (now(), upload_id)); self.server.store.conn.commit()
        except Exception as error:
            self.server.store.conn.rollback()
            compensation_error = None
            try:
                if target.is_file() and not source.exists():
                    target.replace(source)
                elif target.is_file():
                    target.unlink()
            except Exception as rollback_error:
                compensation_error = rollback_error
            try:
                self.server.store.conn.execute("UPDATE upload_sessions SET status='failed',final_file_id=NULL,final_storage_name=NULL,updated_at=? WHERE id=?", (now(), upload_id)); self.server.store.conn.commit()
            except Exception:
                self.server.store.conn.rollback()
            if compensation_error is not None:
                raise RuntimeError(f"No se pudo completar la carga ni restaurar su temporal: {compensation_error}") from error
            raise
        except sqlite3.IntegrityError:
            target.unlink(missing_ok=True); self.server.store.conn.execute("UPDATE upload_sessions SET status='failed',updated_at=? WHERE id=?", (now(), upload_id)); self.server.store.conn.commit(); raise ValueError("El archivo ya existe o el registro no es válido")
        return self.json(201, {"file_id": file_id, "size": upload["expected_size"], "sha256": upload["expected_sha256"]})

    def download(self, file_id):
        user = self.auth()
        if not user:
            return self.json(401, {"error": "SesiÃ³n no vÃ¡lida"})
        if not self.server.store.device_for(self.headers.get("X-Device-ID", ""), user["id"]):
            return self.json(403, {"error": "Dispositivo no vinculado o revocado"})
        conditional = [self.headers.get(name) for name in ("X-Sync-Kind", "X-Sync-Record-ID", "X-Sync-Revision")]
        if any(value is not None for value in conditional):
            if not all(value for value in conditional):
                return self.json(400, {"error": "La descarga condicionada requiere kind, record_id y revisión"})
            kind, record_id, revision_text = conditional
            try:
                revision = int(revision_text)
            except (TypeError, ValueError):
                return self.json(400, {"error": "La revisión condicionada no es válida"})
            with self.server.sync_lock:
                file = self.server.store.conn.execute(
                    "SELECT f.*, sr.revision, sr.payload AS record_payload FROM files f "
                    "JOIN file_records fr ON fr.file_id=f.id AND fr.user_id=f.user_id AND fr.record_id=f.record_id AND fr.kind=f.kind "
                    "JOIN sync_records sr ON sr.user_id=f.user_id AND sr.record_id=f.record_id AND sr.kind=f.kind "
                    "WHERE f.id=? AND f.user_id=? AND f.record_id=? AND f.kind=? AND sr.revision=? AND sr.deleted=0",
                    (file_id, user["id"], record_id, kind, revision),
                ).fetchone()
                if not file:
                    return self.json(409, {"error": "La revisión remota ya no coincide con el conflicto"})
                try:
                    payload_hash = str(json.loads(file["record_payload"]).get("hash", "")).lower()
                except (TypeError, ValueError):
                    payload_hash = ""
                if not payload_hash or payload_hash != file["sha256"].lower():
                    return self.json(409, {"error": "El archivo no coincide con los metadatos de la revisión"})
                target = (self.server.files_root / file["storage_name"]).resolve(); root = self.server.files_root.resolve()
                if root not in target.parents or not target.is_file():
                    return self.json(404, {"error": "Archivo no disponible"})
                self.send_response(200); self.send_header("Content-Type", file["mime"]); self.send_header("Content-Length", str(file["size_bytes"])); self.send_header("X-Content-SHA256", file["sha256"]); self.cors_headers(self.request_origin()); self.end_headers()
                with target.open("rb") as stream:
                    while chunk := stream.read(CHUNK_SIZE): self.wfile.write(chunk)
            return
        file = self.server.store.conn.execute("SELECT * FROM files WHERE id=? AND user_id=?", (file_id, user["id"])).fetchone()
        if not file:
            return self.json(404, {"error": "Archivo no encontrado"})
        target = (self.server.files_root / file["storage_name"]).resolve(); root = self.server.files_root.resolve()
        if root not in target.parents or not target.is_file():
            return self.json(404, {"error": "Archivo no disponible"})
        self.send_response(200); self.send_header("Content-Type", file["mime"]); self.send_header("Content-Length", str(file["size_bytes"])); self.send_header("X-Content-SHA256", file["sha256"]); self.cors_headers(self.request_origin()); self.end_headers()
        with target.open("rb") as stream:
            while chunk := stream.read(CHUNK_SIZE): self.wfile.write(chunk)


def run(host, port, db, allowed_origins=None):
    validate_bind_host(host)
    configured_origins = parse_allowed_origins(allowed_origins)
    server = ThreadingHTTPServer((host, port), Api)
    server.store = Store(db)
    server.allowed_origins = configured_origins
    server.files_root = Path(os.environ.get("LUMINA_API_STORAGE", "lumina-files")).resolve()
    server.sync_lock = threading.RLock()
    server.upload_lock = threading.RLock()
    (server.files_root / "uploads").mkdir(parents=True, exist_ok=True)
    (server.files_root / "files").mkdir(parents=True, exist_ok=True)
    server.store.recover_uploads(server.files_root)
    server.auth_attempts = {}
    server.transfer_attempts = {}
    server.auth_lock = threading.Lock()
    def allow_auth_attempt(address):
        current = now()
        with server.auth_lock:
            attempts = [value for value in server.auth_attempts.get(address, []) if current - value < 900]
            if len(attempts) >= 10:
                server.auth_attempts[address] = attempts
                return False
            attempts.append(current)
            server.auth_attempts[address] = attempts
            return True
    server.allow_auth_attempt = allow_auth_attempt
    def allow_transfer_attempt(address):
        current = now()
        with server.auth_lock:
            attempts = [value for value in server.transfer_attempts.get(address, []) if current - value < 60]
            if len(attempts) >= 120:
                server.transfer_attempts[address] = attempts
                return False
            attempts.append(current)
            server.transfer_attempts[address] = attempts
            return True
    server.allow_transfer_attempt = allow_transfer_attempt
    print(f"Lumina sync API local escuchando en http://{host}:{port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default=os.environ.get("LUMINA_API_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("LUMINA_API_PORT", "8765")))
    parser.add_argument("--db", default=os.environ.get("LUMINA_API_DB", str(Path("lumina-sync.db"))))
    parser.add_argument("--allowed-origin", action="append", dest="allowed_origins", default=None)
    args = parser.parse_args()
    run(args.host, args.port, args.db, args.allowed_origins)
