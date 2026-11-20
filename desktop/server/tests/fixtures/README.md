# Archive verification fixtures

Generated locally using the installed ffmpeg, without external media or downloads:

```sh
ffmpeg -nostdin -v error -f lavfi -i color=c=black:s=32x32:r=1 -t 300 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -movflags +faststart -n archive-video.mp4
ffmpeg -nostdin -v error -f lavfi -i color=c=black:s=32x32:r=1 -t 2 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -movflags +faststart -n archive-clip.mp4
```

The permanent Rust tests serve these real MP4 files over a loopback HTTP listener. They require installed ffprobe and fail rather than silently skipping if it is missing. The episode fixture is 300 seconds at 1 fps, with a front-loaded MP4 index. The clip is two seconds; tests claim a longer metadata runtime to prove independent inspection rejects it. Truncation tests cut the real episode bytes in half.
