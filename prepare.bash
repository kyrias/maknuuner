#!/usr/bin/env bash

set -euo pipefail

mkdir -p assets/

curl -Lo assets/Amiri-1.003.zip https://github.com/aliftype/amiri/releases/download/1.003/Amiri-1.003.zip
unzip assets/Amiri-1.003.zip -d assets/
