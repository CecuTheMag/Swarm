# Downloads the model files of templates/ai/catalog.json into models/ (spec 0017), each
# from its pinned revision on Hugging Face, and checks its size and SHA-256.
#
#   python3 tools/models.py              every model that is not a test model
#   python3 tools/models.py --test       the test models only (CI, browser tests)
#   python3 tools/models.py --fill       record the size and SHA-256 of files the catalog
#                                        does not pin yet (when adding a model)
#
# Files already in models/ with the right hash are not downloaded again.
import hashlib, json, os, sys, tempfile, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG = os.path.join(ROOT, "templates", "ai", "catalog.json")
OUT = os.environ.get("SWARM_MODELS_DIR", os.path.join(ROOT, "models"))


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(dest))
    req = urllib.request.Request(url, headers={"User-Agent": "swarm-models/1"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r, os.fdopen(fd, "wb") as f:
            total = int(r.headers.get("content-length") or 0)
            done, shown = 0, 0
            for chunk in iter(lambda: r.read(1 << 20), b""):
                f.write(chunk)
                done += len(chunk)
                # A line every quarter of a large file.
                if total > 50 << 20 and done * 4 // total > shown:
                    shown = done * 4 // total
                    print(f"  {done >> 20} / {total >> 20} MiB", flush=True)
        os.replace(tmp, dest)
    except BaseException:
        os.unlink(tmp)
        raise


def main():
    args = set(sys.argv[1:])
    fill, test = "--fill" in args, "--test" in args
    with open(CATALOG, encoding="utf-8") as f:
        catalog = json.load(f)
    changed = False
    for model in catalog["models"]:
        if not fill and model["test"] != test:
            continue
        print(model["id"])
        for file in model["files"]:
            dest = os.path.join(OUT, model["id"], file["path"])
            pinned = file["sha256"] is not None
            if pinned and os.path.exists(dest) and sha256_of(dest) == file["sha256"]:
                print(f"  {file['path']}: present")
                continue
            if not pinned and not fill:
                sys.exit(f"  {file['path']} has no SHA-256 in the catalog; run with --fill")
            url = f"https://huggingface.co/{model['id']}/resolve/{model['revision']}/{file['path']}"
            print(f"  {file['path']}: downloading")
            download(url, dest)
            size, digest = os.path.getsize(dest), sha256_of(dest)
            if pinned and (digest != file["sha256"] or size != file["size"]):
                os.unlink(dest)
                sys.exit(f"  {file['path']}: SHA-256 or size differs from the catalog; removed")
            if not pinned:
                file["size"], file["sha256"] = size, digest
                changed = True
                print(f"  {file['path']}: {size} bytes, {digest}")
    if changed:
        with open(CATALOG, "w", encoding="utf-8") as f:
            json.dump(catalog, f, indent=2, ensure_ascii=False)
            f.write("\n")


main()
