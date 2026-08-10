#!/bin/sh

set -eu

usage() {
    printf 'usage: %s OWNER/REPOSITORY vX.Y.Z EXPECTED_COMMIT_SHA\n' "$0" >&2
    exit 2
}

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

[ "$#" -eq 3 ] || usage

repository=$1
tag_name=$2
expected_sha=$3

case $repository in
    */*) ;;
    *) usage ;;
esac

if ! printf '%s\n' "$tag_name" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
    fail "release tag must match vX.Y.Z: $tag_name"
fi

if ! printf '%s\n' "$expected_sha" | grep -Eq '^[0-9a-f]{40}$'; then
    fail "expected commit SHA must be 40 lowercase hexadecimal characters"
fi

if ! command -v gh >/dev/null 2>&1; then
    fail "gh is required to verify the remote release boundary"
fi

# The matching active Ruleset is an external immutability boundary. The
# workflow must fail closed when it has not been configured or cannot be read.
ruleset_ids=$(gh api --paginate \
    "repos/$repository/rulesets?includes_parents=true&targets=tag&per_page=100" \
    --jq '.[] | select(.name == "Immutable release tags" and .enforcement == "active") | .id')

ruleset_count=$(printf '%s\n' "$ruleset_ids" | awk 'NF { count += 1 } END { print count + 0 }')
[ "$ruleset_count" -gt 0 ] || fail 'active "Immutable release tags" Ruleset not found'
[ "$ruleset_count" -eq 1 ] || fail 'multiple active "Immutable release tags" Rulesets found'
ruleset_id=$(printf '%s\n' "$ruleset_ids" | awk 'NF { print; exit }')

ruleset_status=$(gh api \
    "repos/$repository/rulesets/$ruleset_id?includes_parents=true" \
    --jq '
        if .target == "tag"
            and .enforcement == "active"
            and (.conditions.ref_name.include | index("refs/tags/v*")) != null
            and (.conditions.ref_name.exclude | length) == 0
            and ([.rules[].type] | index("creation")) != null
            and ([.rules[].type] | index("update")) != null
            and ([.rules[].type] | index("deletion")) != null
        then "ok"
        else "invalid"
        end')

[ "$ruleset_status" = 'ok' ] || fail \
    '"Immutable release tags" must actively restrict v* creation, update, and deletion'

# Resolve both lightweight and annotated tags. GitHub can nest tag objects, so
# follow a bounded chain until the underlying commit is reached.
object_record=$(gh api "repos/$repository/git/ref/tags/$tag_name" \
    --jq '"\(.object.type) \(.object.sha)"')
depth=0

while :; do
    object_type=${object_record%% *}
    object_sha=${object_record#* }

    case $object_type in
        commit) break ;;
        tag)
            depth=$((depth + 1))
            [ "$depth" -le 8 ] || fail "tag object chain is too deep: $tag_name"
            object_record=$(gh api "repos/$repository/git/tags/$object_sha" \
                --jq '"\(.object.type) \(.object.sha)"')
            ;;
        *) fail "unsupported tag target type: $object_type" ;;
    esac
done

if [ "$object_sha" != "$expected_sha" ]; then
    fail "remote tag $tag_name resolves to $object_sha, expected $expected_sha"
fi

printf 'verified immutable release tag %s at %s\n' "$tag_name" "$object_sha"
