#!/usr/bin/env bash
# Fails (exit 1) when a screen writes a text literal directly, instead of Copy.<Area>.<key>.
# Run it from anywhere: bash content/manga/check-copy.sh
# A deliberate exception ends its line with the comment `// copy: ok`.

cd "$(dirname "$0")/../.." || exit 1

# Print "file:line: text" for each literal that matches $1 and is copy.
# $1: grep -E pattern for a call or argument that holds a literal, e.g. Text("Hi")
# $2: grep -E pattern the literal text must match, once interpolations are removed
# $3...: the files to search
check() {
  local call=$1 copy=$2
  shift 2
  grep -noE "$call" "$@" | while IFS= read -r hit; do
    file=${hit%%:*}
    rest=${hit#*:}
    line=${rest%%:*}
    match=${rest#*:}
    text=${match#*\"}
    text=${text%\"}
    # Drop Swift \(...) and Kotlin $name interpolations: they are values, not words.
    words=$(printf '%s' "$text" | sed -E 's/\\\([^)]*\)//g; s/\$\{?[A-Za-z_][A-Za-z0-9_.]*\}?//g')
    if printf '%s' "$words" | grep -qE "$copy" && ! sed -n "${line}p" "$file" | grep -qE 'copy: ok[[:space:]]*$'; then
      echo "$file:$line: $text"
    fi
  done
}

found=$(
  # iPhone: a literal first argument, or a title or label, in the app's Swift code outside UI/Generated.
  check '(^|[^A-Za-z])(AkiText|AkiButton|Text|Label|Button|\.akiPrompt|\.navigationTitle|\.accessibilityLabel|\.accessibilityHint|AkiRow|AkiSection|AkiEmptyState|AkiHeader)\("[^"]*"' \
    '[A-Za-z].*[A-Za-z]' $(find apps/manga-ios/App -name '*.swift' -not -path '*/UI/Generated/*')

  # Android: a literal first argument or a named text, starting with a capital and with a lowercase letter after it.
  check '(^|[^A-Za-z])((AkiText|AkiButton|Text)\("[^"]*"|(contentDescription|placeholder|title|label|message) = "[^"]*")' \
    '^[A-Z].*[a-z]' $(find apps/manga-android -path '*/src/main/*' -name '*.kt' -not -name 'AkiCopy.kt')
)

if [ -n "$found" ]; then
  echo "$found"
  echo "check-copy: text written directly in a screen. Move it to content/manga/copy/, or end the line with // copy: ok if it is not copy." >&2
  exit 1
fi
echo "check-copy: no text written directly in a screen"
