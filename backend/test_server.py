import json
import hashlib
import threading
import tempfile
import unittest
import urllib.error
import urllib.request
from pathlib import Path
from http.server import ThreadingHTTPServer
from server import Api, Store, password_hash, password_verify


class AccountSecurityTests(unittest.TestCase):
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


class FileTransferTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Api)
        self.http.store = Store(str(Path(self.temp.name) / "sync.db"))
        self.http.files_root = Path(self.temp.name) / "files"
        (self.http.files_root / "uploads").mkdir(parents=True)
        self.http.auth_attempts = {}
        self.http.auth_lock = threading.Lock()
        self.http.allow_auth_attempt = lambda address: True
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


if __name__ == "__main__":
    unittest.main()
