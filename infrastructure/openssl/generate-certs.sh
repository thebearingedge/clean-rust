#!/bin/sh

set -eu

D="${CERT_DIR:-/tmp/certs}"
cd "$D"

if [ -f .done ]; then
  echo "certs already present in $D, skipping"
  exit 0
fi

openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem \
  -days 3650 -subj "/CN=local-dev-ca"

issue() {
  openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$1.key"
  openssl req -new -key "$1.key" -subj "/CN=$2" -out "$1.csr"
  printf 'subjectAltName=%s\nextendedKeyUsage=%s\nbasicConstraints=CA:FALSE\n' "$3" "$4" > "$1.ext"
  openssl x509 -req -in "$1.csr" -CA ca.pem -CAkey ca.key -CAcreateserial \
    -days 825 -extfile "$1.ext" -out "$1.pem"
}

issue broker kafka  "DNS:localhost,DNS:kafka,IP:127.0.0.1" "serverAuth,clientAuth"
issue client client "DNS:localhost"                        "clientAuth"

cat broker.key broker.pem > broker-keystore.pem

rm -f ./*.csr ./*.ext ./*.srl
chmod 644 ./*
touch .done

echo "certs generated in $D"
