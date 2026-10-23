#!/usr/bin/env bash
set -euo pipefail

# Fetch vendored tree-sitter grammars at the revision each language plugin pins in
# src/languages/<lang>/grammar.txt (`upstream = <repo> <branch> <src subdir>`, `rev = <commit sha>`,
# optional `patch = <sed script>`). The build never follows a moving branch head.
# Usage:
#   ./scripts/update-grammars.sh               # fetch every pinned grammar
#   ./scripts/update-grammars.sh json          # fetch only one
#   ./scripts/update-grammars.sh --bump [name] # move the pin to the branch head, then fetch it

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR_DIR="${REPO_ROOT}/vendor/grammars"
LANGUAGES_DIR="${REPO_ROOT}/src/languages"

BUMP=0
if [ "${1:-}" = "--bump" ]; then
    BUMP=1
    shift
fi
TARGET_FILTER="${1:-all}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

field() {
    sed -n "s/^$2 *= *//p" "$1"
}

bump_rev() {
    local manifest="$1" repo="$2" branch="$3" head
    head="$(git ls-remote "${repo}" "refs/heads/${branch}" | cut -f1)"
    [ -n "${head}" ] || { echo "error: ${repo} has no branch ${branch}" >&2; exit 1; }
    if grep -q '^rev' "${manifest}"; then
        sed -i.bak "s/^rev *=.*/rev = ${head}/" "${manifest}" && rm -f "${manifest}.bak"
    else
        sed -i.bak "/^upstream/a\\
rev = ${head}
" "${manifest}" && rm -f "${manifest}.bak"
    fi
}

fetch_rev() {
    local repo="$1" rev="$2" dir="$3"
    git init -q "${dir}"
    git -C "${dir}" remote add origin "${repo}"
    git -C "${dir}" fetch -q --depth 1 origin "${rev}"
    git -C "${dir}" checkout -q FETCH_HEAD
}

update_grammar() {
    local name="$1"
    local manifest="${LANGUAGES_DIR}/${name}/grammar.txt"
    local repo branch upstream_src rev
    read -r repo branch upstream_src <<< "$(field "${manifest}" upstream)"
    if [ "${BUMP}" = 1 ]; then
        bump_rev "${manifest}" "${repo}" "${branch}"
    fi
    rev="$(field "${manifest}" rev)"
    if [ -z "${rev}" ]; then
        echo "error: ${manifest} has no rev; run ./scripts/update-grammars.sh --bump ${name}" >&2
        exit 1
    fi
    local dest_src
    dest_src="$(field "${manifest}" src)"
    dest_src="${dest_src:-src}"

    echo "==> Fetching ${name} (${repo} @ ${rev})..."
    local clone_dir="${TMP_DIR}/${name}"
    fetch_rev "${repo}" "${rev}" "${clone_dir}"

    local src_dir="${clone_dir}/${upstream_src}"
    local dest_dir="${VENDOR_DIR}/${name}/${dest_src}"
    mkdir -p "${dest_dir}/tree_sitter"
    for file in parser.c scanner.c schema.core.c; do
        [ -f "${src_dir}/${file}" ] && cp "${src_dir}/${file}" "${dest_dir}/"
    done
    if [ -d "${src_dir}/tree_sitter" ]; then
        cp -R "${src_dir}/tree_sitter/"* "${dest_dir}/tree_sitter/"
    fi
    for inc in $(field "${manifest}" include); do
        mkdir -p "${VENDOR_DIR}/${name}/${inc}"
        cp -R "${clone_dir}/${inc}/." "${VENDOR_DIR}/${name}/${inc}/"
    done
    while IFS= read -r script; do
        [ -f "${dest_dir}/scanner.c" ] && sed -i.bak "${script}" "${dest_dir}/scanner.c" && rm -f "${dest_dir}/scanner.c.bak"
    done < <(field "${manifest}" patch)

    echo "    Updated ${name} in ${VENDOR_DIR}/${name}"
}

for manifest in "${LANGUAGES_DIR}"/*/grammar.txt; do
    name="$(basename "$(dirname "${manifest}")")"
    grep -q '^upstream' "${manifest}" || continue
    if [ "${TARGET_FILTER}" = "all" ] || [ "${TARGET_FILTER}" = "${name}" ]; then
        update_grammar "${name}"
    fi
done

echo "Done."
