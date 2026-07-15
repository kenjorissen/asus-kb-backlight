from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
MASTER = ASSETS / "asus-kbdlight-master.png"


def main() -> None:
    master = Image.open(MASTER).convert("RGBA")

    # Pillow writes a proper multi-image Windows icon from this RGBA master.
    master.save(
        ASSETS / "asus-kbdlight.ico",
        format="ICO",
        sizes=[(16, 16), (20, 20), (24, 24), (32, 32), (40, 40), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    tray = master.resize((32, 32), Image.Resampling.LANCZOS)
    tray.save(ASSETS / "asus-kbdlight-32.png")
    (ASSETS / "asus-kbdlight-32.rgba").write_bytes(tray.tobytes())

    small = master.resize((16, 16), Image.Resampling.LANCZOS)
    small.save(ASSETS / "asus-kbdlight-16.png")


if __name__ == "__main__":
    main()
