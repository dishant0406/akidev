# Shared by release-ios.sh and release-android.sh. Source it; do not run it.

# A release must match a commit, so stop if tracked files have uncommitted changes.
require_clean_tree() {
  if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "Commit or stash your changes first. A release must match a commit."
    exit 1
  fi
}

# Commit and push the version files, only if the bump changed them. Argument: a label such as "iPhone".
commit_version() {
  git add apps/manga-ios/project.yml apps/manga-android/app/build.gradle.kts
  git diff --cached --quiet && return
  version="$(sed -n 's/.*MARKETING_VERSION: "\(.*\)"/\1/p' apps/manga-ios/project.yml)"
  git commit -q -m "Version $version: $1"
  git push
}

# The shared copy and tokens must match the generated Swift and Kotlin, and no screen may hold text of its own.
check_content() {
  python3 content/manga/generate.py --check
  bash content/manga/check-copy.sh
}
