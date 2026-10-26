import hashlib
import json
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path

from server import Api, Store


class TwoInstallationIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.install_a = root / "installation-a"
        self.install_b = root / "installation-b"
        self.install_a.mkdir(); self.install_b.mkdir()
        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Api)
        self.http.store = Store(str(root / "api.db"))
        self.http.files_root = root / "remote-files"
        (self.http.files_root / "uploads").mkdir(parents=True)
        self.http.sync_lock = threading.RLock()
        self.http.auth_attempts = {}; self.http.transfer_attempts = {}; self.http.auth_lock = threading.Lock()
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
        with urllib.request.urlopen(request) as response:
            return response.status, json.loads(response.read())

    def account(self, email):
        _, result = self.call("/v1/auth/register", {"email": email, "password": "correct horse battery"})
        return result["token"]

    def device(self, token, device_id):
        self.call("/v1/devices", {"id": device_id, "name": device_id, "platform": "windows"}, token)

    def upload(self, token, device_id, record_id, content, name, mime, interrupt=False):
        digest = hashlib.sha256(content).hexdigest()
        kind = "cursor" if name.lower().endswith((".cur", ".ani")) else "wallpaper"
        _, init = self.call("/v1/files/upload/init", {"device_id": device_id, "record_id": record_id, "kind": kind, "name": name, "mime": mime, "size": len(content), "sha256": digest}, token)
        upload_id = init["upload_id"]
        if interrupt:
            first = content[:3]
            self.call(f"/v1/files/upload/{upload_id}", token=token, method="PUT", body=first, headers={"Content-Type": "application/octet-stream", "X-Upload-Offset": "0", "X-Upload-Length": str(len(first))})
        offset = int(self.call(f"/v1/files/upload/{upload_id}", token=token, method="GET")[1]["offset"])
        if offset < len(content):
            self.call(f"/v1/files/upload/{upload_id}", token=token, method="PUT", body=content[offset:], headers={"Content-Type": "application/octet-stream", "X-Upload-Offset": str(offset), "X-Upload-Length": str(len(content) - offset)})
        return self.call(f"/v1/files/upload/{upload_id}/complete", token=token, method="POST")[1]["file_id"], digest

    def test_shared_account_two_installations_lists_files_conflicts_and_isolation(self):
        token_a = self.account("shared@example.com")
        _, login_b = self.call("/v1/auth/login", {"email": "shared@example.com", "password": "correct horse battery"})
        token_b = login_b["token"]
        self.device(token_a, "device-a"); self.device(token_b, "device-b")
        wallpaper = b"\x89PNG\r\n\x1a\ninstallation-a-wallpaper"
        cursor = b"\x00\x00\x02\x00valid-test-cursor"
        for record_id, kind, name, content, mime in (("wall-a", "wallpaper", "a.png", wallpaper, "image/png"), ("cursor-a", "cursor", "a.cur", cursor, "image/x-icon")):
            self.call("/v1/sync", {"device_id": "device-a", "changes": [{"id": record_id, "kind": kind, "base_revision": 0, "payload": {"name": name}}]}, token_a)
            self.upload(token_a, "device-a", record_id, content, name, mime)
        _, remote = self.call("/v1/sync", {"device_id": "device-b", "changes": []}, token_b)
        wallpaper_file = next(item for item in remote["files"] if item["record_id"] == "wall-a")
        request = urllib.request.Request(self.base + f"/v1/files/{wallpaper_file['id']}", headers={"Authorization": f"Bearer {token_b}", "X-Device-ID": "device-b"})
        with urllib.request.urlopen(request) as response: restored = response.read()
        (self.install_b / "wall-a.png").write_bytes(restored)
        (self.install_a / "wall-a.png").write_bytes(wallpaper)
        self.assertEqual(hashlib.sha256((self.install_a / "wall-a.png").read_bytes()).hexdigest(), hashlib.sha256(restored).hexdigest())
        list_payload = {"name": "Orden compartido", "wallpaper_ids": ["wall-a", "cursor-a"]}
        _, first = self.call("/v1/sync", {"device_id": "device-a", "changes": [{"id": "list-a", "kind": "playlist", "base_revision": 0, "payload": list_payload}]}, token_a)
        _, from_a = self.call("/v1/sync", {"device_id": "device-b", "changes": []}, token_b)
        received = next(item for item in from_a["records"] if item["record_id"] == "list-a")
        self.assertEqual(received["payload"]["wallpaper_ids"], ["wall-a", "cursor-a"])
        revision = received["revision"]
        self.call("/v1/sync", {"device_id": "device-b", "changes": [{"id": "list-a", "kind": "playlist", "base_revision": revision, "payload": {"name": "Orden B", "wallpaper_ids": ["cursor-a", "wall-a"]}}]}, token_b)
        _, conflict = self.call("/v1/sync", {"device_id": "device-a", "changes": [{"id": "list-a", "kind": "playlist", "base_revision": revision, "payload": {"name": "Orden A", "wallpaper_ids": ["wall-a", "cursor-a"]}}]}, token_a)
        self.assertEqual(conflict["conflicts"][0]["id"], "list-a")
        self.call("/v1/sync", {"device_id": "device-a", "changes": [{"id": "wall-a", "kind": "wallpaper", "base_revision": 1, "deleted": True, "payload": {}}]}, token_a)
        _, after_delete = self.call("/v1/sync", {"device_id": "device-b", "changes": []}, token_b)
        self.assertTrue(any(item["record_id"] == "wall-a" and item["deleted"] for item in after_delete["records"]))
        other = self.account("other@example.com"); self.device(other, "other-device")
        with self.assertRaises(urllib.error.HTTPError) as error:
            request = urllib.request.Request(self.base + f"/v1/files/{wallpaper_file['id']}", headers={"Authorization": f"Bearer {other}", "X-Device-ID": "other-device"})
            urllib.request.urlopen(request)
        self.assertEqual(error.exception.code, 404)
        self.call("/v1/sync", {"device_id": "device-a", "changes": [{"id": "resume", "kind": "wallpaper", "base_revision": 0, "payload": {"name": "resume.png"}}]}, token_a)
        resume_content = wallpaper + b"-resume"
        _, file_id = self.upload(token_a, "device-a", "resume", resume_content, "resume.png", "image/png", interrupt=True)
        self.assertTrue(file_id)


if __name__ == "__main__":
    unittest.main()
