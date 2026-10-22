#!/usr/bin/env bash
set -euo pipefail

# Update vendored tree-sitter grammars from the upstream repositories each language plugin declares
# in src/languages/<lang>/grammar.txt (`upstream = <repo> <branch> <src subdir>`, optional `patch = <sed script>`).
# Usage:
#   ./scripts/update-grammars.sh         # updates every plugin that declares an upstream
#   ./scripts/update-grammars.sh json    # updates only one

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR_DIR="${REPO_ROOT}/vendor/grammars"
LANGUAGES_DIR="${REPO_ROOT}/src/languages"

TARGET_FILTER="${1:-all}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

field() {
    sed -n "s/^$2 *= *//p" "$1"
}

update_grammar() {
    local name="$1"
    local manifest="${LANGUAGES_DIR}/${name}/grammar.txt"
    local repo branch upstream_src
    read -r repo branch upstream_src <<< "$(field "${manifest}" upstream)"
    local dest_src
    dest_src="$(field "${manifest}" src)"
    dest_src="${dest_src:-src}"

    echo "==> Fetching ${name} (${repo} @ ${branch})..."
    local clone_dir="${TMP_DIR}/${name}"
    git clone --depth 1 --single-branch -b "${branch}" "${repo}" "${clone_dir}" 2>/dev/null

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
