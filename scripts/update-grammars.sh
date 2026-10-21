#!/usr/bin/env bash
set -euo pipefail

# Update vendored tree-sitter grammars from authoritative upstream repositories.
# Usage:
#   ./scripts/update-grammars.sh         # updates all 11 grammars
#   ./scripts/update-grammars.sh json    # updates only json

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR_DIR="${REPO_ROOT}/vendor/grammars"

# Format: name|repo_url|branch|src_subdir
GRAMMARS=(
    "dart|https://github.com/UserNobody14/tree-sitter-dart|master|src"
    "kotlin|https://github.com/fwcd/tree-sitter-kotlin|main|src"
    "php|https://github.com/tree-sitter/tree-sitter-php|master|special_php"
    "gdscript|https://github.com/PrestonKnopp/tree-sitter-gdscript|master|src"
    "toml|https://github.com/tree-sitter-grammars/tree-sitter-toml|master|src"
    "markdown|https://github.com/tree-sitter-grammars/tree-sitter-markdown|split_parser|tree-sitter-markdown/src"
    "sql|https://github.com/DerekStride/tree-sitter-sql|gh-pages|src"
    "brightscript|https://github.com/ajdelcimmuto/tree-sitter-brightscript|main|src"
    "cobol|https://github.com/yutaro-sakamoto/tree-sitter-cobol|main|src"
)

TARGET_FILTER="${1:-all}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

update_grammar() {
    local name="$1"
    local repo="$2"
    local branch="$3"
    local src_subdir="$4"

    echo "==> Fetching ${name} (${repo} @ ${branch})..."
    local clone_dir="${TMP_DIR}/${name}"
    git clone --depth 1 --single-branch -b "${branch}" "${repo}" "${clone_dir}" 2>/dev/null

    if [ "${name}" = "php" ]; then
        local dest_php_src="${VENDOR_DIR}/php/php/src"
        local dest_common="${VENDOR_DIR}/php/common"
        mkdir -p "${dest_php_src}/tree_sitter" "${dest_common}"

        cp "${clone_dir}/php/src/parser.c" "${dest_php_src}/"
        [ -f "${clone_dir}/php/src/scanner.c" ] && cp "${clone_dir}/php/src/scanner.c" "${dest_php_src}/"
        cp -R "${clone_dir}/php/src/tree_sitter/"* "${dest_php_src}/tree_sitter/"
        cp "${clone_dir}/common/scanner.h" "${dest_common}/"
    else
        local src_dir="${clone_dir}/${src_subdir}"
        local dest_dir="${VENDOR_DIR}/${name}/src"
        mkdir -p "${dest_dir}/tree_sitter"

        cp "${src_dir}/parser.c" "${dest_dir}/"
        [ -f "${src_dir}/scanner.c" ] && cp "${src_dir}/scanner.c" "${dest_dir}/"
        [ -f "${src_dir}/schema.core.c" ] && cp "${src_dir}/schema.core.c" "${dest_dir}/"
        if [ -d "${src_dir}/tree_sitter" ]; then
            cp -R "${src_dir}/tree_sitter/"* "${dest_dir}/tree_sitter/"
        fi
        if [ "${name}" = "gdscript" ] && [ -f "${dest_dir}/scanner.c" ]; then
            sed -i.bak 's/return (Delimiter){0};/Delimiter d; d.flags = 0; return d;/' "${dest_dir}/scanner.c" && rm -f "${dest_dir}/scanner.c.bak"
        fi
        if [ "${name}" = "cobol" ] && [ -f "${dest_dir}/scanner.c" ]; then
            sed -i.bak 's/char \*keyword_pointer\[number_of_words\];/char *keyword_pointer[16];/g; s/bool continue_check\[number_of_words\];/bool continue_check[16];/g' "${dest_dir}/scanner.c" && rm -f "${dest_dir}/scanner.c.bak"
        fi
    fi

    echo "    Updated ${name} in ${VENDOR_DIR}/${name}"
}

for entry in "${GRAMMARS[@]}"; do
    IFS='|' read -r name repo branch src_subdir <<< "${entry}"
    if [ "${TARGET_FILTER}" = "all" ] || [ "${TARGET_FILTER}" = "${name}" ]; then
        update_grammar "${name}" "${repo}" "${branch}" "${src_subdir}"
    fi
done

echo "Done."
