#!/bin/bash
PREV_TAG=$(gh release list --limit 1 --json tagName --jq '.[0].tagName // empty')

NOTES_FILE=$(mktemp)
CACHE_SERVER_VERSION="5.5.5"

echo "## What's Changed" > "$NOTES_FILE"
echo "" >> "$NOTES_FILE"

if [ -n "$PREV_TAG" ]; then
  git log "${PREV_TAG}..${CACHE_SERVER_VERSION}" \
    --pretty=format:"* %s (%h)" \
    --grep="^Release " --invert-grep >> "$NOTES_FILE"
  echo "" >> "$NOTES_FILE"
  echo "" >> "$NOTES_FILE"
  echo "**Full Changelog**: https://github.com/${{ github.repository }}/compare/${PREV_TAG}...${CACHE_SERVER_VERSION}" >> "$NOTES_FILE"
else
  git log "${CACHE_SERVER_VERSION}" \
    --pretty=format:"* %s (%h)" >> "$NOTES_FILE"
fi

echo $NOTES_FILE