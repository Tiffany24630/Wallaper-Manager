import json
import hashlib
import sqlite3
import threading
import tempfile
import unittest
import urllib.error
import urllib.request
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


if __name__ == "__main__":
    unittest.main()
