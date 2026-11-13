#!/usr/bin/env bash
# Make the self-signed code signing certificate lgterm.app is signed with, once. macOS keeps
# folder and privacy grants for an app only while its signer stays the same; the throwaway
# signature from `codesign --sign -` changes every build, so each build asked again.
set -euo pipefail

name="lgterm local signing"
keychain="$HOME/Library/Keychains/login.keychain-db"
if security find-identity -p codesigning "$keychain" | grep -q "$name"; then
  exit 0
fi

echo "Creating the \"$name\" certificate in your login keychain. macOS asks for your password once to trust it for code signing." >&2
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
cat > "$dir/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $name
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$dir/cert.cnf" \
  -keyout "$dir/key.pem" -out "$dir/cert.pem" 2>/dev/null
/usr/bin/openssl pkcs12 -export -inkey "$dir/key.pem" -in "$dir/cert.pem" \
  -out "$dir/id.p12" -passout pass:lgterm
security import "$dir/id.p12" -k "$keychain" -P lgterm -T /usr/bin/codesign >/dev/null
security add-trusted-cert -p codeSign -k "$keychain" "$dir/cert.pem"
