#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
video_root="$repo_root/examples/proposal-flow-video/video"

cd "$video_root"

if [[ ! -d node_modules ]]; then
  npm ci
fi

npm run render
printf '\nRendered MP4: %s/out/proposal-flow-video.mp4\n' "$video_root"
