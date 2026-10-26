import json
import hashlib
import sqlite3
import threading
import tempfile
import unittest
import urllib.error
import urllib.request
from types import SimpleNamespace
from pathlib import Path
from http.server import ThreadingHTTPServer
from server import Api, Store, is_loopback_host, parse_allowed_origins, password_hash, password_verify, validate_bind_host


class AccountSecurityTests(unittest.TestCase):
    def test_network_mode_is_loopback_only_and_cors_origins_are_explicit(self):
        self.assertTrue(is_loopback_host("127.0.0.1"))
        self.assertTrue(is_loopback_host("::1"))
        self.assertFalse(is_loopback_host("0.0.0.0"))
        validate_bind_host("localhost")
        with self.assertRaises(ValueError):
            validate_bind_host("192.0.2.10")
        with self.assertRaises(ValueError):
            parse_allowed_origins("*")

    def test_password_is_scrypt_and_verifies(self):
        encoded = password_hash("correct horse battery")
        self.assertTrue(encoded.startswith("scrypt$"))
        self.assertTrue(password_verify("correct horse battery", encoded))
        self.assertFalse(password_verify("wrong", encoded))
        self.assertNotEqual(encoded, password_hash("correct horse battery"))

    def test_store_isolates_users_and_keeps_device_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            store = Store(str(Path(directory) / "sync.db"))
            store.conn.execute("INSERT INTO users VALUES('u1','a@example.com',?,1)", (password_hash("1234567890"),))
            store.conn.execute("INSERT INTO users VALUES('u2','b@example.com',?,1)", (password_hash("1234567890"),))
            store.conn.execute("INSERT INTO devices VALUES('device','u1','Desktop','windows',NULL,NULL)")
            store.conn.commit()
            self.assertEqual(store.session_user("missing"), None)
            self.assertEqual(store.conn.execute("SELECT user_id FROM devices WHERE id='device'").fetchone()[0], "u1")
            store.close()

    def test_store_migrates_existing_record_ids_to_typed_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sync.db"
            legacy = sqlite3.connect(path)
            legacy.executescript(
                """
                CREATE TABLE users(id TEXT PRIMARY KEY, email TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, created_at INTEGER NOT NULL);
                CREATE TABLE sync_records(
                    user_id TEXT NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL,
                    payload TEXT NOT NULL, revision INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                    deleted INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(user_id, record_id)
                );
                CREATE TABLE devices(
                    id TEXT PRIMARY KEY, user_id TEXT NOT NULL, name TEXT NOT NULL, platform TEXT NOT NULL,
                    last_sync_at INTEGER, revoked_at INTEGER
                );
                INSERT INTO users VALUES('u1','legacy@example.com','not-used',1);
                INSERT INTO sync_records VALUES('u1','same-id','wallpaper','{}',1,1,0);
                INSERT INTO devices VALUES('legacy-device','u1','Legacy','windows',NULL,NULL);
                """
            )
            legacy.commit(); legacy.close()

            store = Store(str(path))
            store.conn.execute(
                "INSERT INTO sync_records VALUES(?,?,?,?,?,?,?)",
                ("u1", "same-id", "cursor", "{}", 1, 2, 0),
            )
            store.conn.commit()
            self.assertEqual(
                store.conn.execute("SELECT COUNT(*) FROM sync_records WHERE user_id='u1' AND record_id='same-id'").fetchone()[0],
                2,
            )
            self.assertEqual(store.conn.execute("SELECT COUNT(*) FROM devices WHERE id='legacy-device'").fetchone()[0], 1)
            store.close()

    def test_store_migrates_file_relationships_to_typed_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sync.db"
            legacy = sqlite3.connect(path)
            legacy.executescript(
                """
                CREATE TABLE users(id TEXT PRIMARY KEY, email TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL, created_at INTEGER NOT NULL);
                CREATE TABLE files(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL, original_name TEXT NOT NULL, mime TEXT NOT NULL, extension TEXT NOT NULL, size_bytes INTEGER NOT NULL, sha256 TEXT NOT NULL, storage_name TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL);
                CREATE TABLE file_records(file_id TEXT NOT NULL, user_id TEXT NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL, PRIMARY KEY(file_id, record_id));
                INSERT INTO users VALUES('u1','legacy@example.com','not-used',1);
                INSERT INTO files VALUES('f1','u1','same-id','wallpaper','a.png','image/png','.png',8,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','files/u1/f1.png',1);
                INSERT INTO file_records VALUES('f1','u1','same-id','wallpaper');
                """
            )
            legacy.commit(); legacy.close()
            store = Store(str(path))
            store.conn.execute("INSERT INTO file_records VALUES(?,?,?,?)", ("f1", "u1", "same-id", "cursor"))
            store.conn.commit()
            rows = store.conn.execute("SELECT kind FROM file_records WHERE file_id='f1' AND record_id='same-id' ORDER BY kind").fetchall()
            self.assertEqual([row[0] for row in rows], ["cursor", "wallpaper"])
            store.close()
            reopened = Store(str(path))
            self.assertEqual(reopened.conn.execute("SELECT COUNT(*) FROM file_records").fetchone()[0], 2)
            reopened.close()

    def test_sync_batch_rolls_back_when_a_later_change_is_invalid(self):
        with tempfile.TemporaryDirectory() as directory:
            store = Store(str(Path(directory) / "sync.db"))
            store.conn.execute("INSERT INTO users VALUES('u1','batch@example.com',?,1)", (password_hash("1234567890"),))
            store.conn.execute("INSERT INTO devices VALUES('d1','u1','Test','windows',NULL,NULL)")
            store.conn.commit()
            class Harness(Api):
                def auth(self): return store.conn.execute("SELECT * FROM users WHERE id='u1'").fetchone()
                def json(self, status, payload): return status, payload
            harness = Harness.__new__(Harness)
            harness.server = SimpleNamespace(store=store, sync_lock=threading.RLock())
            with self.assertRaises(ValueError):
                harness._sync_locked({"device_id": "d1", "changes": [
                    {"id": "first", "kind": "wallpaper", "base_revision": 0, "payload": {}},
                    {"id": "second", "kind": "unsupported", "base_revision": 0, "payload": {}},
                ]})
            self.assertIsNone(store.conn.execute("SELECT 1 FROM sync_records WHERE record_id='first'").fetchone())
            store.close()

    def test_expired_upload_cleanup_is_idempotent_and_preserves_active_or_invalid_rows(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root / "uploads").mkdir()
            store = Store(str(root / "sync.db"))
            self.addCleanup(store.close)
            store.conn.execute("INSERT INTO users VALUES('u1','cleanup@example.com',?,1)", (password_hash("1234567890"),))
            expired_id = "a" * 32; active_id = "b" * 32; invalid_id = "c" * 32
            for upload_id, temp_name, updated_at, status in (
                (expired_id, expired_id + ".part", 1, "uploading"),
                (active_id, active_id + ".part", 150, "uploading"),
                (invalid_id, "../outside.part", 1, "uploading"),
                ("d" * 32, "d" * 32 + ".part", 1, "complete"),
            ):
                store.conn.execute("INSERT INTO upload_sessions(user_id,device_id,record_id,kind,original_name,mime,expected_size,expected_sha256,temp_name,received_bytes,status,created_at,updated_at,final_file_id,final_storage_name,id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", ("u1", "d1", "r1", "wallpaper", "a.png", "image/png", 8, "a" * 64, temp_name, 0, status, 1, updated_at, None, None, upload_id))
                if temp_name.endswith(".part") and not temp_name.startswith("../"):
                    (root / "uploads" / temp_name).touch()
            store.conn.commit()
            self.assertEqual(store.cleanup_expired_uploads(root, now_value=200, ttl=100), 1)
            self.assertEqual(store.cleanup_expired_uploads(root, now_value=200, ttl=100), 0)
            self.assertIsNone(store.conn.execute("SELECT 1 FROM upload_sessions WHERE id=?", (expired_id,)).fetchone())
            self.assertIsNotNone(store.conn.execute("SELECT 1 FROM upload_sessions WHERE id=?", (active_id,)).fetchone())
            self.assertIsNotNone(store.conn.execute("SELECT 1 FROM upload_sessions WHERE id=?", (invalid_id,)).fetchone())
            self.assertTrue((root / "uploads" / (active_id + ".part")).exists())
            store.close()

    def test_recovery_reconciles_intent_states_idempotently(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root / "uploads").mkdir(); (root / "files" / "u1").mkdir(parents=True)
            store = Store(str(root / "sync.db"))
            self.addCleanup(store.close)
            store.conn.execute("INSERT INTO users VALUES('u1','recover@example.com',?,1)", (password_hash("1234567890"),))
            content = b"\x89PNG\r\n\x1a\nrecovery"; digest = hashlib.sha256(content).hexdigest()

            def add_session(upload_id, status, final_id, final_name, temp_name):
                store.conn.execute("INSERT INTO upload_sessions(user_id,device_id,record_id,kind,original_name,mime,expected_size,expected_sha256,temp_name,received_bytes,status,created_at,updated_at,final_file_id,final_storage_name,id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", ("u1", "d1", upload_id, "wallpaper", "recover.png", "image/png", len(content), digest, temp_name, len(content), status, 1, 1, final_id, final_name, upload_id))

            before_move = "a" * 32
            add_session(before_move, "publishing", before_move, f"files/u1/{before_move}.png", f"{before_move}.part")
            (root / "uploads" / f"{before_move}.part").write_bytes(content)

            after_move = "b" * 32
            add_session(after_move, "publishing", after_move, f"files/u1/{after_move}.png", f"{after_move}.part")
            (root / "files" / "u1" / f"{after_move}.png").write_bytes(content)

            after_sqlite = "c" * 32
            add_session(after_sqlite, "publishing", after_sqlite, f"files/u1/{after_sqlite}.png", f"{after_sqlite}.part")
            (root / "files" / "u1" / f"{after_sqlite}.png").write_bytes(content)
            store.conn.execute("INSERT INTO files VALUES(?,?,?,?,?,?,?,?,?,?,?)", (after_sqlite, "u1", after_sqlite, "wallpaper", "recover.png", "image/png", ".png", len(content), digest, f"files/u1/{after_sqlite}.png", 1))
            store.conn.execute("INSERT INTO file_records VALUES(?,?,?,?)", (after_sqlite, "u1", after_sqlite, "wallpaper"))
            store.conn.execute("UPDATE upload_sessions SET prepared_resolution_id='recovered-resolution',prepared_revision=1 WHERE id=?", (after_sqlite,))
            store.conn.execute("INSERT INTO sync_resolutions VALUES(?,?,?,?,?,?,?,?)", ("u1", "recovered-resolution", "wallpaper", after_sqlite, 1, "keep_local", 2, 1))

            invalid = "d" * 32
            add_session(invalid, "publishing", invalid, f"files/u1/{invalid}.png", f"{invalid}.part")
            (root / "files" / "u1" / f"{invalid}.png").write_bytes(b"not-an-image")
            store.conn.commit()

            self.assertEqual(store.recover_uploads(root), 2)
            self.assertEqual(store.conn.execute("SELECT status FROM upload_sessions WHERE id=?", (before_move,)).fetchone()[0], "uploading")
            self.assertEqual(store.conn.execute("SELECT status FROM upload_sessions WHERE id=?", (after_move,)).fetchone()[0], "complete")
            self.assertEqual(store.conn.execute("SELECT status FROM upload_sessions WHERE id=?", (after_sqlite,)).fetchone()[0], "complete")
            self.assertEqual(store.conn.execute("SELECT status FROM upload_sessions WHERE id=?", (invalid,)).fetchone()[0], "failed")
            self.assertFalse((root / "files" / "u1" / f"{invalid}.png").exists())
            self.assertEqual(store.recover_uploads(root), 0)
            self.assertEqual(store.conn.execute("SELECT COUNT(*) FROM file_records WHERE record_id=?", (after_move,)).fetchone()[0], 1)
            store.close()

    def test_upload_intent_columns_are_added_to_legacy_schema(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "legacy.db"
            legacy = sqlite3.connect(path)
            legacy.executescript("""
                CREATE TABLE upload_sessions(
                  id TEXT PRIMARY KEY, user_id TEXT NOT NULL, device_id TEXT NOT NULL,
                  record_id TEXT NOT NULL, kind TEXT NOT NULL, original_name TEXT NOT NULL,
                  mime TEXT NOT NULL, expected_size INTEGER NOT NULL, expected_sha256 TEXT NOT NULL,
                  temp_name TEXT NOT NULL UNIQUE, received_bytes INTEGER NOT NULL DEFAULT 0,
                  status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
                );
            """)
            legacy.commit(); legacy.close()
            store = Store(str(path))
            columns = {row[1] for row in store.conn.execute("PRAGMA table_info(upload_sessions)").fetchall()}
            self.assertTrue({"final_file_id", "final_storage_name"}.issubset(columns))
            store.close()

    def test_device_identity_is_scoped_to_account(self):
        with tempfile.TemporaryDirectory() as directory:
            store = Store(str(Path(directory) / "sync.db"))
            store.conn.execute("INSERT INTO users VALUES('u1','a@example.com',?,1)", (password_hash("1234567890"),))
            store.conn.execute("INSERT INTO users VALUES('u2','b@example.com',?,1)", (password_hash("1234567890"),))
            store.conn.execute("INSERT INTO devices VALUES('shared-device','u1','Android','android',NULL,NULL)")
            store.conn.execute("INSERT INTO devices VALUES('shared-device','u2','Android','android',NULL,NULL)")
            store.conn.commit()
            self.assertEqual(store.conn.execute("SELECT COUNT(*) FROM devices WHERE id='shared-device'").fetchone()[0], 2)
            self.assertIsNotNone(store.device_for("shared-device", "u1"))
            self.assertIsNotNone(store.device_for("shared-device", "u2"))
            store.close()


class FileTransferTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Api)
        self.http.store = Store(str(Path(self.temp.name) / "sync.db"))
        self.http.files_root = Path(self.temp.name) / "files"
        (self.http.files_root / "uploads").mkdir(parents=True)
        self.http.allowed_origins = {"https://app.example"}
        self.http.sync_lock = threading.RLock()
        self.http.auth_attempts = {}
        self.http.auth_lock = threading.Lock()
        self.http.allow_auth_attempt = lambda address: True
        self.http.allow_transfer_attempt = lambda address: True
        self.thread = threading.Thread(target=self.http.serve_forever, daemon=True); self.thread.start()
        self.base = f"http://127.0.0.1:{self.http.server_port}"

    def tearDown(self):
        self.http.shutdown(); self.http.server_close(); self.http.store.close(); self.thread.join(timeout=2); self.temp.cleanup()

    def call(self, path, payload=None, token=None, method=None, body=None, headers=None):
        data = json.dumps(payload).encode() if payload is not None else body
        request = urllib.request.Request(self.base + path, data=data, method=method or ("POST" if payload is not None else "GET"), headers={"Content-Type": "application/json", **(headers or {})})
        if token: request.add_header("Authorization", f"Bearer {token}")
        with urllib.request.urlopen(request) as response: return response.status, json.loads(response.read())

    def account(self, email):
        _, result = self.call("/v1/auth/register", {"email": email, "password": "correct horse battery"})
        token = result["token"]; self.call("/v1/devices", {"id": email, "name": "Test", "platform": "windows"}, token)
        return token

    def test_upload_download_hash_and_cross_user_isolation(self):
        token = self.account("a@example.com"); content = b"\x89PNG\r\n\x1a\nsmall-test"; digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "a@example.com", "changes": [{"id": "wall-1", "kind": "wallpaper", "base_revision": 0, "payload": {"name": "A"}}]}, token)
        _, init = self.call("/v1/files/upload/init", {"device_id": "a@example.com", "record_id": "wall-1", "kind": "wallpaper", "name": "wall.png", "mime": "image/png", "size": len(content), "sha256": digest}, token)
        upload = init["upload_id"]
        self.call(f"/v1/files/upload/{upload}", token=token, method="PUT", body=content, headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        _, completed = self.call(f"/v1/files/upload/{upload}/complete", token=token, method="POST")
        status, download = self.call("/v1/sync", {"device_id": "a@example.com", "changes": []}, token)
        self.assertEqual(status, 200); self.assertEqual(download["files"][0]["sha256"], digest)
        request = urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers={"Authorization": "Bearer " + token, "X-Device-ID": "a@example.com"})
        with urllib.request.urlopen(request) as response: self.assertEqual(response.read(), content)
        other = self.account("b@example.com")
        with self.assertRaises(urllib.error.HTTPError) as error:
            request = urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers={"Authorization": "Bearer " + other, "X-Device-ID": "b@example.com"})
            urllib.request.urlopen(request)
        self.assertEqual(error.exception.code, 404)

    def test_cors_allowlist_preflight_and_auth_error_do_not_reflect_origin_or_token(self):
        allowed = urllib.request.Request(
            self.base + "/v1/auth/session",
            headers={"Origin": "https://app.example", "Authorization": "Bearer super-secret-token"},
        )
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(allowed)
        self.assertEqual(error.exception.code, 401)
        self.assertEqual(error.exception.headers.get("Access-Control-Allow-Origin"), "https://app.example")
        self.assertNotIn("*", error.exception.headers.get("Access-Control-Allow-Origin", ""))
        self.assertNotIn("super-secret-token", error.exception.read().decode())

        denied = urllib.request.Request(
            self.base + "/v1/auth/session",
            headers={"Origin": "https://evil.example"},
        )
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(denied)
        self.assertEqual(error.exception.code, 403)
        self.assertIsNone(error.exception.headers.get("Access-Control-Allow-Origin"))

        preflight = urllib.request.Request(
            self.base + "/v1/sync",
            method="OPTIONS",
            headers={
                "Origin": "https://app.example",
                "Access-Control-Request-Method": "POST",
                "Access-Control-Request-Headers": "Authorization, Content-Type, X-Device-ID, X-Sync-Revision",
            },
        )
        with urllib.request.urlopen(preflight) as response:
            self.assertEqual(response.status, 204)
            self.assertEqual(response.headers.get("Access-Control-Allow-Origin"), "https://app.example")
            self.assertIn("authorization", response.headers.get("Access-Control-Allow-Headers", "").lower())
            self.assertIn("x-sync-revision", response.headers.get("Access-Control-Allow-Headers", "").lower())

        denied_preflight = urllib.request.Request(
            self.base + "/v1/sync",
            method="OPTIONS",
            headers={"Origin": "https://evil.example", "Access-Control-Request-Method": "POST"},
        )
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(denied_preflight)
        self.assertEqual(error.exception.code, 403)

    def test_conditional_download_requires_expected_revision_and_typed_hash(self):
        token = self.account("conditional@example.com")
        content = b"\x89PNG\r\n\x1a\nconditional"; digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "conditional@example.com", "changes": [{"id": "same-id", "kind": "wallpaper", "base_revision": 0, "payload": {"hash": digest}}]}, token)
        _, init = self.call("/v1/files/upload/init", {"device_id": "conditional@example.com", "record_id": "same-id", "kind": "wallpaper", "name": "wall.png", "mime": "image/png", "size": len(content), "sha256": digest}, token)
        self.call(f"/v1/files/upload/{init['upload_id']}", token=token, method="PUT", body=content, headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        _, completed = self.call(f"/v1/files/upload/{init['upload_id']}/complete", token=token, method="POST")
        headers = {"Authorization": "Bearer " + token, "X-Device-ID": "conditional@example.com", "Origin": "https://app.example", "X-Sync-Kind": "wallpaper", "X-Sync-Record-ID": "same-id", "X-Sync-Revision": "1"}
        request = urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers=headers)
        with urllib.request.urlopen(request) as response:
            self.assertEqual(response.read(), content)
            self.assertEqual(response.headers.get("Access-Control-Allow-Origin"), "https://app.example")

        wrong_kind = {**headers, "X-Sync-Kind": "cursor"}
        with self.assertRaises(urllib.error.HTTPError) as error:
            urllib.request.urlopen(urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers=wrong_kind))
        self.assertEqual(error.exception.code, 409)

        self.call("/v1/sync", {"device_id": "conditional@example.com", "changes": [{"id": "same-id", "kind": "wallpaper", "base_revision": 1, "payload": {"hash": digest, "changed": True}}]}, token)
        request = urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers=headers)
        with self.assertRaises(urllib.error.HTTPError) as error: urllib.request.urlopen(request)
        self.assertEqual(error.exception.code, 409)

        self.http.store.conn.execute("UPDATE sync_records SET payload=? WHERE record_id='same-id' AND kind='wallpaper'", (json.dumps({"hash": "0" * 64}),)); self.http.store.conn.commit()
        with self.assertRaises(urllib.error.HTTPError) as error:
            request = urllib.request.Request(self.base + f"/v1/files/{completed['file_id']}", headers={**headers, "X-Sync-Revision": "2"}); urllib.request.urlopen(request)
        self.assertEqual(error.exception.code, 409)

    def test_invalid_hash_and_incomplete_transfer_are_rejected(self):
        token = self.account("c@example.com")
        self.call("/v1/sync", {"device_id": "c@example.com", "changes": [{"id": "wall-2", "kind": "wallpaper", "base_revision": 0, "payload": {}}]}, token)
        _, init = self.call("/v1/files/upload/init", {"device_id": "c@example.com", "record_id": "wall-2", "kind": "wallpaper", "name": "wall.png", "mime": "image/png", "size": 12, "sha256": "0" * 64}, token)
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call(f"/v1/files/upload/{init['upload_id']}/complete", token=token, method="POST")
        self.assertEqual(error.exception.code, 409)

    def test_upload_commit_failure_restores_staging_and_allows_clean_retry(self):
        token = self.account("rollback@example.com")
        content = b"\x89PNG\r\n\x1a\nrollback"; digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "rollback@example.com", "changes": [{"id": "rollback-wall", "kind": "wallpaper", "base_revision": 0, "payload": {}}]}, token)
        _, init = self.call("/v1/files/upload/init", {"device_id": "rollback@example.com", "record_id": "rollback-wall", "kind": "wallpaper", "name": "rollback.png", "mime": "image/png", "size": len(content), "sha256": digest}, token)
        upload_id = init["upload_id"]
        self.call(f"/v1/files/upload/{upload_id}", token=token, method="PUT", body=content, headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        self.http.before_upload_persist = lambda: (_ for _ in ()).throw(RuntimeError("controlled persistence failure"))
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call(f"/v1/files/upload/{upload_id}/complete", token=token, method="POST")
        self.assertEqual(error.exception.code, 500)
        self.assertFalse([path for path in (Path(self.temp.name) / "files" / "files").rglob("*") if path.is_file()])
        staging = Path(self.temp.name) / "files" / "uploads" / f"{upload_id}.part"
        self.assertTrue(staging.is_file())
        self.assertEqual(staging.read_bytes(), content)
        self.assertEqual(self.http.store.conn.execute("SELECT status FROM upload_sessions WHERE id=?", (upload_id,)).fetchone()[0], "failed")
        self.http.before_upload_persist = None
        _, retry = self.call("/v1/files/upload/init", {"device_id": "rollback@example.com", "record_id": "rollback-wall", "kind": "wallpaper", "name": "rollback.png", "mime": "image/png", "size": len(content), "sha256": digest}, token)
        self.call(f"/v1/files/upload/{retry['upload_id']}", token=token, method="PUT", body=content, headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        _, completed = self.call(f"/v1/files/upload/{retry['upload_id']}/complete", token=token, method="POST")
        self.assertEqual(self.http.store.conn.execute("SELECT COUNT(*) FROM files WHERE record_id='rollback-wall'").fetchone()[0], 1)
        self.assertIsNotNone(self.http.store.conn.execute("SELECT id FROM files WHERE id=?", (completed["file_id"],)).fetchone())

    def test_file_metadata_limits_and_path_traversal_are_rejected(self):
        token = self.account("d@example.com")
        self.call("/v1/sync", {"device_id": "d@example.com", "changes": [{"id": "wall-3", "kind": "wallpaper", "base_revision": 0, "payload": {}}]}, token)
        for payload in (
            {"name": "..\\escape.png", "mime": "image/png", "size": 4},
            {"name": "empty.png", "mime": "image/png", "size": 0},
            {"name": "bad.gif", "mime": "image/gif", "size": 4},
        ):
            payload.update({"device_id": "d@example.com", "record_id": "wall-3", "kind": "wallpaper", "sha256": "0" * 64})
            with self.assertRaises(urllib.error.HTTPError) as error: self.call("/v1/files/upload/init", payload, token)
            self.assertEqual(error.exception.code, 400)

    def test_revision_conflict_list_order_and_device_revocation(self):
        token = self.account("lists@example.com")
        first = {"id": "list-1", "kind": "playlist", "base_revision": 0, "payload": {"name": "Orden A", "wallpaper_ids": ["wall-1", "wall-2"]}}
        _, result = self.call("/v1/sync", {"device_id": "lists@example.com", "changes": [first]}, token)
        self.assertEqual(result["records"][0]["payload"]["wallpaper_ids"], ["wall-1", "wall-2"])
        conflict = {"id": "list-1", "kind": "playlist", "base_revision": 0, "payload": {"name": "Cambio concurrente", "wallpaper_ids": ["wall-2", "wall-1"]}}
        _, result = self.call("/v1/sync", {"device_id": "lists@example.com", "changes": [conflict]}, token)
        self.assertEqual(result["conflicts"][0]["id"], "list-1")
        self.call("/v1/devices/lists@example.com", method="DELETE", token=token)
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync", {"device_id": "lists@example.com", "changes": []}, token)
        self.assertEqual(error.exception.code, 403)

    def test_same_record_id_isolated_by_kind_for_updates_conflicts_and_deletes(self):
        token = self.account("typed@example.com")
        changes = [
            {"id": "same-id", "kind": "wallpaper", "base_revision": 0, "payload": {"name": "Fondo"}},
            {"id": "same-id", "kind": "playlist", "base_revision": 0, "payload": {"name": "Lista"}},
            {"id": "same-id", "kind": "cursor", "base_revision": 0, "payload": {"name": "Cursor"}},
        ]
        _, initial = self.call("/v1/sync", {"device_id": "typed@example.com", "changes": changes}, token)
        self.assertEqual(len([record for record in initial["records"] if record["record_id"] == "same-id"]), 3)

        self.call("/v1/sync", {"device_id": "typed@example.com", "changes": [
            {"id": "same-id", "kind": "wallpaper", "base_revision": 1, "payload": {"name": "Fondo actualizado"}},
            {"id": "same-id", "kind": "playlist", "base_revision": 1, "payload": {"name": "Lista actualizada"}},
            {"id": "same-id", "kind": "cursor", "base_revision": 1, "payload": {"name": "Cursor actualizado"}},
        ]}, token)
        _, conflict = self.call("/v1/sync", {"device_id": "typed@example.com", "changes": [
            {"id": "same-id", "kind": "playlist", "base_revision": 1, "payload": {"name": "Lista obsoleta"}},
        ]}, token)
        self.assertEqual(conflict["conflicts"][0]["kind"], "playlist")
        for kind, payload in (("wallpaper", {"name": "Fondo obsoleto"}), ("cursor", {"name": "Cursor obsoleto"})):
            _, typed_conflict = self.call("/v1/sync", {"device_id": "typed@example.com", "changes": [
                {"id": "same-id", "kind": kind, "base_revision": 1, "payload": payload},
            ]}, token)
            self.assertEqual(typed_conflict["conflicts"][0]["kind"], kind)
            self.assertEqual(typed_conflict["conflicts"][0]["id"], "same-id")

        _, deleted = self.call("/v1/sync", {"device_id": "typed@example.com", "changes": [
            {"id": "same-id", "kind": "wallpaper", "base_revision": 2, "deleted": True, "payload": {}},
        ]}, token)
        records = {(record["kind"], record["record_id"]): record for record in deleted["records"]}
        self.assertTrue(records[("wallpaper", "same-id")]["deleted"])
        self.assertFalse(records[("playlist", "same-id")]["deleted"])
        self.assertFalse(records[("cursor", "same-id")]["deleted"])

    def test_sync_resolution_is_typed_idempotent_and_rejects_stale_or_missing_files(self):
        token = self.account("resolve@example.com")
        other = self.account("other-resolve@example.com")
        same_id = "shared-resolution-id"
        self.call("/v1/sync", {"device_id": "resolve@example.com", "changes": [
            {"id": same_id, "kind": "wallpaper", "base_revision": 0, "payload": {"name": "Wall", "hash": "0" * 64, "size_bytes": 8}},
            {"id": same_id, "kind": "cursor", "base_revision": 0, "payload": {"name": "Cursor"}},
            {"id": same_id, "kind": "playlist", "base_revision": 0, "payload": {"name": "Playlist"}},
        ]}, token)

        remote_request = {"device_id": "resolve@example.com", "kind": "playlist", "record_id": same_id,
                          "expected_revision": 1, "action": "keep_remote", "resolution_id": "remote-playlist-1"}
        status, resolved = self.call("/v1/sync/resolve", remote_request, token)
        self.assertEqual(status, 200)
        self.assertEqual(resolved["kind"], "playlist")
        self.assertFalse(resolved["idempotent"])
        _, repeated = self.call("/v1/sync/resolve", remote_request, token)
        self.assertTrue(repeated["idempotent"])

        local_request = {"device_id": "resolve@example.com", "kind": "cursor", "record_id": same_id,
                         "expected_revision": 1, "action": "keep_local", "resolution_id": "local-cursor-1",
                         "local_payload": {"name": "Cursor local"}}
        _, local_result = self.call("/v1/sync/resolve", local_request, token)
        self.assertEqual(local_result["revision"], 2)
        stale_request = {**local_request, "resolution_id": "local-cursor-stale"}
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", stale_request, token)
        self.assertEqual(error.exception.code, 409)

        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", {**remote_request, "device_id": "other-resolve@example.com", "resolution_id": "other-account-id"}, other)
        self.assertEqual(error.exception.code, 409)

        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", {"device_id": "resolve@example.com", "kind": "wallpaper", "record_id": same_id,
                                             "expected_revision": 1, "action": "keep_remote", "resolution_id": "missing-wall-file"}, token)
        self.assertEqual(error.exception.code, 409)

        rows = {(row["kind"], row["record_id"]): row for row in self.http.store.conn.execute(
            "SELECT kind,record_id,revision,payload FROM sync_records WHERE user_id=(SELECT id FROM users WHERE email=?)", ("resolve@example.com",))}
        self.assertEqual(rows[("playlist", same_id)]["revision"], 1)
        self.assertEqual(json.loads(rows[("playlist", same_id)]["payload"])["name"], "Playlist")
        self.assertEqual(rows[("cursor", same_id)]["revision"], 2)
        self.assertEqual(json.loads(rows[("cursor", same_id)]["payload"])["name"], "Cursor local")

        self.call("/v1/sync", {"device_id": "resolve@example.com", "changes": [{
            "id": same_id, "kind": "playlist", "base_revision": 1, "deleted": True, "payload": {},
        }]}, token)
        tombstone_request = {"device_id": "resolve@example.com", "kind": "playlist", "record_id": same_id,
                             "expected_revision": 2, "action": "keep_local", "resolution_id": "local-tombstone-invalid",
                             "local_payload": {}}
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", tombstone_request, token)
        self.assertEqual(error.exception.code, 409)
        _, tombstone = self.call("/v1/sync/resolve", {**tombstone_request, "resolution_id": "local-tombstone-valid", "deleted": True}, token)
        self.assertTrue(tombstone["deleted"])

    def test_remote_resolution_validates_typed_file_before_confirming(self):
        token = self.account("remote-file@example.com")
        record_id = "remote-wallpaper"
        content = b"\x89PNG\r\n\x1a\nfixture"
        digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "remote-file@example.com", "changes": [{
            "id": record_id, "kind": "wallpaper", "base_revision": 0,
            "payload": {"hash": digest, "size_bytes": len(content), "name": "fixture.png"},
        }]}, token)
        user_id = self.http.store.conn.execute("SELECT id FROM users WHERE email=?", ("remote-file@example.com",)).fetchone()[0]
        file_id = "remote-file-id"
        relative = f"files/{user_id}/{file_id}.png"
        target = self.http.files_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)
        self.http.store.conn.execute(
            "INSERT INTO files VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            (file_id, user_id, record_id, "wallpaper", "fixture.png", "image/png", ".png", len(content), digest, relative, 1),
        )
        self.http.store.conn.execute("INSERT INTO file_records VALUES(?,?,?,?)", (file_id, user_id, record_id, "wallpaper"))
        self.http.store.conn.commit()

        status, result = self.call("/v1/sync/resolve", {"device_id": "remote-file@example.com", "kind": "wallpaper",
            "record_id": record_id, "expected_revision": 1, "action": "keep_remote", "resolution_id": "valid-remote-file"}, token)
        self.assertEqual(status, 200)
        self.assertEqual(result["file"]["sha256"], digest)
        self.assertEqual(result["file"]["kind"], "wallpaper")

        target.write_bytes(b"corrupt")
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", {"device_id": "remote-file@example.com", "kind": "wallpaper",
                                             "record_id": record_id, "expected_revision": 1, "action": "keep_remote",
                                             "resolution_id": "corrupt-remote-file"}, token)
        self.assertEqual(error.exception.code, 409)

    def test_keep_local_wallpaper_publishes_prepared_binary_idempotently(self):
        token = self.account("prepared-local@example.com")
        record_id = "prepared-wallpaper"
        content = b"\x89PNG\r\n\x1a\nlocal-conflict-bytes"
        digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "prepared-local@example.com", "changes": [{
            "id": record_id, "kind": "wallpaper", "base_revision": 0,
            "payload": {"name": "remote.png", "hash": "0" * 64, "size_bytes": 8},
        }]}, token)
        _, init = self.call("/v1/files/upload/init", {
            "device_id": "prepared-local@example.com", "record_id": record_id, "kind": "wallpaper",
            "name": "local.png", "mime": "image/png", "size": len(content), "sha256": digest,
            "defer_resolution": True, "resolution_id": "prepared-resolution", "expected_revision": 1,
        }, token)
        upload_id = init["upload_id"]
        self.call(f"/v1/files/upload/{upload_id}", token=token, method="PUT", body=content,
                  headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        _, prepared = self.call(f"/v1/files/upload/{upload_id}/complete", token=token, method="POST")
        self.assertTrue(prepared["prepared"])
        self.assertEqual(self.http.store.conn.execute("SELECT COUNT(*) FROM files").fetchone()[0], 0)
        request = {"device_id": "prepared-local@example.com", "kind": "wallpaper", "record_id": record_id,
                   "expected_revision": 1, "action": "keep_local", "resolution_id": "prepared-resolution",
                   "prepared_upload_id": upload_id, "local_payload": {"name": "local.png", "hash": digest, "size_bytes": len(content)}}
        _, resolved = self.call("/v1/sync/resolve", request, token)
        self.assertTrue(resolved["resolved"])
        self.assertEqual(resolved["revision"], 2)
        file_row = self.http.store.conn.execute("SELECT * FROM files WHERE user_id=(SELECT id FROM users WHERE email=?)", ("prepared-local@example.com",)).fetchone()
        self.assertEqual((self.http.files_root / file_row["storage_name"]).read_bytes(), content)
        _, retry_upload = self.call("/v1/files/upload/init", {
            "device_id": "prepared-local@example.com", "record_id": record_id, "kind": "wallpaper",
            "name": "local.png", "mime": "image/png", "size": len(content), "sha256": digest,
            "defer_resolution": True, "resolution_id": "prepared-resolution", "expected_revision": 1,
        }, token)
        self.assertTrue(retry_upload["prepared"])
        self.assertEqual(retry_upload["status"], "complete")
        _, repeated = self.call("/v1/sync/resolve", request, token)
        self.assertTrue(repeated["idempotent"])
        self.assertEqual(self.http.store.conn.execute("SELECT COUNT(*) FROM files").fetchone()[0], 1)

    def test_keep_local_binary_failure_restores_prepared_upload(self):
        token = self.account("prepared-failure@example.com")
        content = b"\x89PNG\r\n\x1a\nfailure-fixture"
        digest = hashlib.sha256(content).hexdigest()
        self.call("/v1/sync", {"device_id": "prepared-failure@example.com", "changes": [{"id": "failure-wall", "kind": "wallpaper", "base_revision": 0, "payload": {"hash": "0" * 64, "size_bytes": 8}}]}, token)
        _, init = self.call("/v1/files/upload/init", {"device_id": "prepared-failure@example.com", "record_id": "failure-wall", "kind": "wallpaper", "name": "failure.png", "mime": "image/png", "size": len(content), "sha256": digest, "defer_resolution": True, "resolution_id": "failure-resolution", "expected_revision": 1}, token)
        self.call(f"/v1/files/upload/{init['upload_id']}", token=token, method="PUT", body=content, headers={"Content-Type": "application/octet-stream", "Content-Length": str(len(content)), "X-Upload-Offset": "0"})
        self.call(f"/v1/files/upload/{init['upload_id']}/complete", token=token, method="POST")
        self.http.before_resolution_persist = lambda: (_ for _ in ()).throw(RuntimeError("controlled"))
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.call("/v1/sync/resolve", {"device_id": "prepared-failure@example.com", "kind": "wallpaper", "record_id": "failure-wall", "expected_revision": 1, "action": "keep_local", "resolution_id": "failure-resolution", "prepared_upload_id": init["upload_id"], "local_payload": {"hash": digest, "size_bytes": len(content)}}, token)
        self.assertEqual(error.exception.code, 500)
        self.http.before_resolution_persist = None
        row = self.http.store.conn.execute("SELECT status,temp_name FROM upload_sessions WHERE id=?", (init["upload_id"],)).fetchone()
        self.assertEqual(row["status"], "prepared")
        self.assertEqual((self.http.files_root / "uploads" / row["temp_name"]).read_bytes(), content)
        self.assertEqual(self.http.store.conn.execute("SELECT COUNT(*) FROM sync_resolutions").fetchone()[0], 0)

if __name__ == "__main__":
    unittest.main()
