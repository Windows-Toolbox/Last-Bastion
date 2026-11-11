"""Copies TAB's sounds into ours: assets\\sounds\\ (explained in assets\\README.md).

TAB's sounds are copyrighted - only for us, on this PC (assets\\sounds\\ is in .gitignore). This script makes them anew
from your own extracted copy of the game:

    python tools\\tab_sounds.py [assets folder]

`assets folder`: the `_assets` folder of the extracted game (its `Sounds\\`).

What it does:
1. Takes TAB's folders as they are: `Effects` (shots, buildings working, zombies, explosions ...), `UnitVoices`
   ("Yes", "What", "Attack" of every unit), `Notifications` (the officer: "Infected from the north" ...), `Interface`
   (clicks, windows), `Ambient` (forest, wind, the horde far away - one per landscape), `Music`.
2. Leaves out what we don't build (docs\\tab-assets.md): the heroes' voices (`HA_`, `HB_`) and the mutant.
   The campaign's sounds and the trains' are in (for the editor's maps and scripts).
3. macroquad only plays `.ogg` and `.wav` - TAB's `.mp3` become `.ogg` (with ffmpeg, which has to be installed). A space
   in a name becomes `_` (`Zombie Ambience.mp3` -> `Zombie_Ambience.ogg`).
A file that's already there and not older than TAB's is skipped - running it again only does what's new.
"""

import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

from tab_terrain import DEFAULT_TAB

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "sounds")

# Names that start with one of these are left out (heroes, mutant).
SKIP = ("HA_", "HB_", "Mutant")
# ffmpeg's Vorbis quality (0-10): 4 is about 128 kbit/s, like TAB's mp3s.
QUALITY = "4"


def target(folder, name):
    stem, ext = os.path.splitext(name)
    ext = ext.lower()
    return os.path.join(OUT, folder, stem.strip().replace(" ", "_") + (".ogg" if ext == ".mp3" else ext))


def one(job):
    """Copies or converts one file - returns an error text or None."""
    source, dest = job
    if os.path.isfile(dest) and os.path.getmtime(dest) >= os.path.getmtime(source):
        return None
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    if source.lower().endswith(".mp3"):
        run = subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", source, "-vn", "-c:a", "libvorbis", "-q:a", QUALITY, dest],
                             capture_output=True, text=True)
        return None if run.returncode == 0 else f"{source}: {run.stderr.strip()}"
    shutil.copyfile(source, dest)
    return None


def main():
    tab = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TAB
    sounds = os.path.join(tab, "Sounds")
    if not os.path.isdir(sounds):
        sys.exit(f"No Sounds folder in {tab} - give the _assets folder of the extracted game.")
    if shutil.which("ffmpeg") is None:
        sys.exit("ffmpeg not found - it turns TAB's mp3s into ogg (winget install Gyan.FFmpeg).")
    jobs, skipped = [], 0
    for folder in sorted(os.listdir(sounds)):
        path = os.path.join(sounds, folder)
        if not os.path.isdir(path):
            continue
        for name in sorted(os.listdir(path)):
            if not name.lower().endswith((".mp3", ".ogg", ".wav")):
                continue
            if name.startswith(SKIP):
                skipped += 1
                continue
            jobs.append((os.path.join(path, name), target(folder, name)))
    print(f"From {sounds} to {os.path.abspath(OUT)}: {len(jobs)} sounds ({skipped} left out) ...")
    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
        errors = [e for e in pool.map(one, jobs) if e]
    for e in errors:
        print("  " + e)
    for folder in sorted(os.listdir(OUT)):
        files = os.listdir(os.path.join(OUT, folder))
        size = sum(os.path.getsize(os.path.join(OUT, folder, f)) for f in files)
        print(f"  {folder}: {len(files)} files, {size / 1e6:.0f} MB")


if __name__ == "__main__":
    main()
