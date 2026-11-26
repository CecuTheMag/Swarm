# mnist14.bin

Derived from the MNIST database of handwritten digits by Yann LeCun, Corinna Cortes
and Christopher J. C. Burges, licensed under CC BY-SA 3.0. This file is a derivative
under the same licence: each 28 × 28 image averaged over 2 × 2 blocks (rounded), the
first 10 000 images of the training set and the first 2 000 of the test set, made by
`scripts/pack_mnist.py`.

Layout: 12 000 × 196 bytes of pixels (training images, then validation images), then
12 000 labels in the same order. SHA-256 `b66ab11fb7cb11263a568b394201c2d0e9bb88a85508abbdedef04d68713c717`.
