#!/usr/bin/env bash
# Copies the three finished competition documents to the repository root, where a reader finds them first:
# the documentation, the presentation and the brochure. Rebuild the sources first (see docs/competition).
set -euo pipefail
cd "$(dirname "$0")/.."
cp "docs/competition/documentation/Рояк.pdf"        "Рояк-документация.pdf"
cp "docs/competition/presentation/Рояк.pdf"         "Рояк-презентация.pdf"
cp "docs/competition/brochure/out/Рояк-диплянка.pdf" "Рояк-диплянка.pdf"
ls -la Рояк-*.pdf
