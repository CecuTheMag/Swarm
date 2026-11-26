# Packs MNIST into templates/nn/mnist14.bin: 14x14 (2x2 mean, rounded), 10 000 training
# images from MNIST's training set and 2 000 validation images from its test set.
# Layout: training pixels, validation pixels, training labels, validation labels.
#
#   python3 scripts/pack_mnist.py <folder with the four MNIST .gz files> templates/nn/mnist14.bin
#
# The files (from any MNIST mirror, e.g. storage.googleapis.com/cvdf-datasets/mnist/) and
# their MD5 sums: train-images-idx3-ubyte.gz f68b3c2dcbeaaa9fbdd348bbdeb94873,
# train-labels-idx1-ubyte.gz d53e105ee54ea40749a09fcbcd1e9432,
# t10k-images-idx3-ubyte.gz 9fb629c4189551a2d022fa330f9573f3,
# t10k-labels-idx1-ubyte.gz ec29112dd5afa0611ce80d1b7f02629c. The output's SHA-256 is
# kernels::nn::mnist::SHA256.
import gzip, struct, sys, hashlib, collections
raw, out = sys.argv[1], sys.argv[2]
def images(name, n):
    with gzip.open(f"{raw}/{name}", "rb") as f:
        magic, count, rows, cols = struct.unpack(">IIII", f.read(16))
        assert magic == 2051 and rows == cols == 28 and count >= n
        data = f.read(n * 784)
    res = bytearray()
    for k in range(n):
        img = data[k * 784:(k + 1) * 784]
        for y in range(14):
            for x in range(14):
                s = img[(2*y)*28 + 2*x] + img[(2*y)*28 + 2*x + 1] + img[(2*y+1)*28 + 2*x] + img[(2*y+1)*28 + 2*x + 1]
                res.append((s + 2) >> 2)
    return bytes(res)
def labels(name, n):
    with gzip.open(f"{raw}/{name}", "rb") as f:
        magic, count = struct.unpack(">II", f.read(8))
        assert magic == 2049 and count >= n
        return f.read(n)
tx, vx = images("train-images-idx3-ubyte.gz", 10000), images("t10k-images-idx3-ubyte.gz", 2000)
ty, vy = labels("train-labels-idx1-ubyte.gz", 10000), labels("t10k-labels-idx1-ubyte.gz", 2000)
blob = tx + vx + ty + vy
open(out, "wb").write(blob)
print(len(blob), hashlib.sha256(blob).hexdigest())
print("train classes", sorted(collections.Counter(ty).items()))
print("val classes", sorted(collections.Counter(vy).items()))
