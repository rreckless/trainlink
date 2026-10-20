"""Locomotive setup: per-locomotive function labels, DCC address changes, and pictures"""

import pytest

from app.api import locomotives as locomotives_api

PNG = b"\x89PNG\r\n\x1a\n" + b"\x00" * 32
JPEG = b"\xff\xd8\xff" + b"\x00" * 32
WEBP = b"RIFF" + b"\x00\x00\x00\x00" + b"WEBP" + b"\x00" * 16


@pytest.fixture
def image_dir(tmp_path, monkeypatch):
    monkeypatch.setattr(locomotives_api, "IMAGE_DIR", tmp_path)
    return tmp_path


def _create(client, name="Class 37", address=3):
    return client.post(
        "/api/locomotives",
        json={"name": name, "dcc_address": address, "locomotive_type": "diesel"},
    ).json()["id"]


def _labels(count=20, **overrides):
    labels = [{"label": f"Fn {index}", "momentary": False} for index in range(count)]
    labels[0] = {"label": "Light", "momentary": False}
    labels[1] = {"label": "Horn", "momentary": True}
    for index, value in overrides.items():
        labels[int(index)] = value
    return labels


def test_new_locomotive_shows_default_function_labels(client):
    loco = client.get(f"/api/locomotives/{_create(client)}").json()
    assert len(loco["function_labels"]) == 20
    assert loco["function_labels"][0] == {"label": "Light", "momentary": False}
    assert loco["function_labels"][5] == {"label": "F5", "momentary": False}
    assert loco["image_url"] is None


def test_function_labels_are_saved_per_locomotive(client):
    loco_id = _create(client)
    response = client.patch(f"/api/locomotives/{loco_id}", json={"function_labels": _labels()})
    assert response.status_code == 200
    body = response.json()
    assert body["function_labels"][1] == {"label": "Horn", "momentary": True}
    assert client.get(f"/api/locomotives/{loco_id}").json()["function_labels"][1]["label"] == "Horn"


@pytest.mark.parametrize("labels", [_labels(19), _labels(21)])
def test_function_labels_must_cover_all_twenty_buttons(client, labels):
    loco_id = _create(client)
    assert client.patch(f"/api/locomotives/{loco_id}", json={"function_labels": labels}).status_code == 422


def test_blank_function_label_is_refused(client):
    loco_id = _create(client)
    labels = _labels(**{"0": {"label": "   ", "momentary": False}})
    assert client.patch(f"/api/locomotives/{loco_id}", json={"function_labels": labels}).status_code == 422


def test_label_too_long_is_refused(client):
    loco_id = _create(client)
    labels = _labels(**{"0": {"label": "x" * 25, "momentary": False}})
    assert client.patch(f"/api/locomotives/{loco_id}", json={"function_labels": labels}).status_code == 422


def test_dcc_address_can_be_changed_and_is_checked(client):
    loco_id = _create(client)
    assert client.patch(f"/api/locomotives/{loco_id}", json={"dcc_address": 12}).json()["dcc_address"] == 12
    assert client.patch(f"/api/locomotives/{loco_id}", json={"dcc_address": 0}).status_code == 400


def test_dcc_address_used_by_another_locomotive_is_refused(client):
    _create(client, "Class 37", 3)
    other = _create(client, "Class 47", 4)
    assert client.patch(f"/api/locomotives/{other}", json={"dcc_address": 3}).status_code == 409


def test_picture_upload_is_stored_and_served(client, image_dir):
    loco_id = _create(client)
    response = client.post(
        f"/api/locomotives/{loco_id}/image",
        files={"file": ("loco.png", PNG, "image/png")},
    )
    assert response.status_code == 200
    assert response.json()["image_url"] == f"/api/locomotives/{loco_id}/image"
    assert (image_dir / f"{loco_id}.png").read_bytes() == PNG
    served = client.get(f"/api/locomotives/{loco_id}/image")
    assert served.status_code == 200
    assert served.headers["content-type"] == "image/png"


@pytest.mark.parametrize(
    "content_type, data",
    [("image/jpeg", JPEG), ("image/webp", WEBP)],
)
def test_jpeg_and_webp_are_accepted(client, image_dir, content_type, data):
    loco_id = _create(client)
    response = client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco", data, content_type)})
    assert response.status_code == 200


def test_picture_of_the_wrong_type_is_refused(client, image_dir):
    loco_id = _create(client)
    response = client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.gif", b"GIF89a", "image/gif")})
    assert response.status_code == 400


def test_picture_with_a_fake_signature_is_refused(client, image_dir):
    loco_id = _create(client)
    response = client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.png", b"not a png", "image/png")})
    assert response.status_code == 400


def test_picture_over_two_megabytes_is_refused(client, image_dir):
    loco_id = _create(client)
    big = PNG + b"\x00" * (2 * 1024 * 1024)
    response = client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.png", big, "image/png")})
    assert response.status_code == 413


def test_new_picture_replaces_the_old_one(client, image_dir):
    loco_id = _create(client)
    client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.png", PNG, "image/png")})
    client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.jpg", JPEG, "image/jpeg")})
    assert not (image_dir / f"{loco_id}.png").exists()
    assert (image_dir / f"{loco_id}.jpg").exists()


def test_picture_can_be_removed(client, image_dir):
    loco_id = _create(client)
    client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.png", PNG, "image/png")})
    response = client.delete(f"/api/locomotives/{loco_id}/image")
    assert response.json()["image_url"] is None
    assert client.get(f"/api/locomotives/{loco_id}/image").status_code == 404
    assert not (image_dir / f"{loco_id}.png").exists()


def test_picture_for_unknown_locomotive_is_404(client, image_dir):
    response = client.post("/api/locomotives/999/image", files={"file": ("loco.png", PNG, "image/png")})
    assert response.status_code == 404


def test_deleting_a_locomotive_removes_its_picture(client, image_dir):
    loco_id = _create(client)
    client.post(f"/api/locomotives/{loco_id}/image", files={"file": ("loco.png", PNG, "image/png")})
    assert (image_dir / f"{loco_id}.png").exists()
    assert client.delete(f"/api/locomotives/{loco_id}").status_code == 204
    assert not (image_dir / f"{loco_id}.png").exists()
