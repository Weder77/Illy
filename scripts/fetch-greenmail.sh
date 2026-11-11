#!/usr/bin/env bash
# Télécharge le serveur mail de test GreenMail (utilisé par les tests d'intégration).
set -euo pipefail
cd "$(dirname "$0")/.."
DEST=target/greenmail/greenmail-standalone.jar
if [ -f "$DEST" ]; then exit 0; fi
mkdir -p target/greenmail
curl -fsSL -o "$DEST.part" \
  https://repo1.maven.org/maven2/com/icegreen/greenmail-standalone/2.1.15/greenmail-standalone-2.1.15.jar
mv "$DEST.part" "$DEST"
