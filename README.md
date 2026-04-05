# Zed with VSCode style Git Panel / Features

I wanted to use Zed because VSCode/Cursor became too bloated. The Git panel is unuseable for me. So this is a fork that fixes it.

[**Download here**](https://github.com/firatoezcan/zed/releases)

Or clone the repo and ask Claude/Codex/AI assistant of your choice to build it for your system

## Changes
- Adding separate Staged/Unstaged areas inside the Git panel, viewing the diff for a single file when clicked instead of buffer and looking at previous changes to a file with arrow buttons: https://github.com/firatoezcan/zed/pull/1

> [!CAUTION]
> This is slow because it was a debug build!! It's not actually that slow otherwise I wouldnt use it myself lol

https://github.com/user-attachments/assets/d6c52cb9-b933-4061-8691-6786a2ffca00

To get the same behavior as in the video set this ✨ new ✨ setting:

```json
{
  "git_panel": {
    "single_file_diff": true
  }
}
```
