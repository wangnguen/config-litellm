# ccl: Claude Code × LiteLLM

Run `ccl` in a project folder to pick the model and effort Claude Code uses. The model list comes straight from your LiteLLM gateway, filtered by your key. `ccl` writes `.claude/settings.local.json` for you, so there is no file to edit by hand.

```
key: chung33  |  gateway: litellm-proxy-...azurefd.net  |  folder: my-project

Switch Model

  Search: █   5/8
          ────────────────────────────

    claude-opus-5-5-vn
    claude-opus-5-vn
    claude-opus-4-8-vn
    claude-sonnet-5-5-vn
  > claude-sonnet-5-vn    (current)
    claude-sonnet-4-6-vn
    claude-haiku-5-5-vn
    claude-haiku-4-5-vn

  Effort  ◂  ●─────────●─────────◉─────────○─────────○  ▸
            low     medium     high      xhigh      max
          Deep reasoning for complex problems

  1M context  on   Tab to switch

  ↑↓ model   ←→ effort   Tab 1M   type to search   Enter save   Esc quit
```

## Install

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/wangnguen/config-litellm/main/install.ps1 | iex
```

**macOS / Linux**

```bash
curl -fsSL https://raw.githubusercontent.com/wangnguen/config-litellm/main/install.sh | bash
```

The script downloads the binary into `~/.ccl/bin` and adds that folder to your PATH. Open a new terminal afterwards.

To build it yourself: `cargo install --git https://github.com/wangnguen/config-litellm ccl`

## Getting started

```bash
cd my-project
ccl
```

On the first run, `ccl` asks for your virtual key (input is hidden) and checks it against the gateway. The key is saved once the gateway accepts it. The model picker then opens. Press Enter, then start a new Claude Code session.

If `~/.ccc/keys.json` from ccc is still on the machine, the first run imports the keys in it.

## The model picker

| Key | Action |
|---|---|
| Type | Filter models, for example `opus 4` |
| `↑` `↓` | Select a model |
| `←` `→` | Change effort |
| `Tab` | Turn 1M context on or off (Sonnet, Opus) |
| `Enter` | Save |
| `Esc` | Clear the search; press again to quit |

You only pick the main model. The `opus`, `sonnet` and `haiku` slots fill themselves with the newest model of each family that your key can use. An empty slot makes its `/model` alias fail with a 403, so `ccl` always fills all four.

## Commands

| Command | What it does |
|---|---|
| `ccl` | Show the key and gateway, then open the model and effort picker |
| `ccl ls` | List the models your key can use and which slot uses each |
| `ccl check` | Call each slot's model for real (1 token) and report OK or the error, with latency |
| `ccl check --all` | Call every model your key can use |
| `ccl key` | List saved keys (`★` default, `●` used by this folder) |
| `ccl key add <name>` | Save a new key (hidden input, checked against the gateway) |
| `ccl key use <name>` | Use a key for this folder and make it the default; slots are adjusted to the models that key can use |
| `ccl key rm <name>` | Delete a key (asks first) |
| `ccl config` | Print `~/.ccl/config.json` |
| `ccl config edit` | Open `config.json` in your editor |
| `ccl config path` | Print the paths of ccl's files |
| `ccl show` | Print this folder's settings, key masked |
| `ccl doctor` | Look for anything overriding the settings (managed policy, shell variables, `~/.claude/settings.json`) |
| `ccl update` | Update to the latest release |
| `ccl uninstall` | Remove ccl and every saved key |

## Gateway adds or removes a model: no code change

Model ids are **not hardcoded**. Every run, `ccl` asks the gateway (`GET /v1/models`), so new models show up and removed ones disappear on their own. To adjust anything, edit `~/.ccl/config.json` (`ccl config edit`):

```json
{
  "base_url": "https://litellm-proxy-ep-cncyfugmcnadc6g4.a02.azurefd.net",
  "model_suffix": "-vn",
  "hidden_models": ["claude-opus-4-8-vn"],
  "slots": { "opus": "auto", "sonnet": "auto", "haiku": "claude-haiku-4-5-vn" },
  "long_context": true,
  "long_context_families": ["sonnet", "opus"],
  "effort_levels": [
    { "id": "low", "description": "Quick answers with light reasoning — fastest" },
    { "id": "high", "description": "Deep reasoning for complex problems" }
  ],
  "default_effort": "high"
}
```

| Key | Meaning |
|---|---|
| `base_url` | Address of the LiteLLM gateway |
| `model_suffix` | Show only models ending with this suffix (`-vn`). Set `""` to show all |
| `hidden_models` | Models to hide even when the gateway lists them |
| `slots` | `auto` = newest model of the same family. Put a model id to pin that slot |
| `long_context`, `long_context_families` | Turn `[1m]` on by default, and for which model families |
| `effort_levels` | The levels on the effort slider, lowest to highest, each with a one-line description |
| `default_effort` | Effort for a folder that has none set |

Any key left out falls back to its default. If the JSON is broken, `ccl` reports the error and does not overwrite your file.

Each folder keeps the models it was given until you run `ccl` there again. After the gateway gains a model, run `ccl` and press Enter in each project to move its `auto` slots to the newest one.

## Settings template

A folder without `.claude/settings.local.json` gets one created from `~/.ccl/template.json`. The default template turns on gateway model discovery, turns off nonessential traffic, and **pre-allows** the `Write`, `Bash`, `Read`, `Edit`, `Glob`, `Grep`, `Agent`, `WebFetch` and `WebSearch` tools (same as ccc's template). If you want Claude Code to ask before running commands, delete those lines from `template.json`.

`ccl` only writes the variables it manages (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, the 4 model slots, `effortLevel`). Anything else you add to the project's settings (hooks, permissions, ...) is kept.

## Troubleshooting

- **403 "key not allowed to access model"**: run `ccl check`; it shows which slot uses a model your key may not use. Run `ccl` to pick again.
- **403 `team_model_access_denied`**: your key allows the model but its LiteLLM team does not. `/v1/models` still lists it, so `ccl` offers it. Ask the gateway admin to add the model to your team, or hide it meanwhile with `hidden_models` in `~/.ccl/config.json`.
- **Claude Code ignores the model change**: start a new session. If that does not help, run `ccl doctor` to see what overrides it (managed policy, `.claude/settings.json`, environment variables).
- **"cannot reach the gateway"**: check `base_url` with `ccl config`.

## Security

- The key is entered hidden and masked when printed (`sk-…f3c9`), so screen sharing is safe.
- `.claude/settings.local.json` (the file holding the key) is added to `.gitignore` automatically when the folder is a git repo. Only that file is added, not the whole `.claude/` folder.
- Keys are stored in plaintext in `~/.ccl/keys.json`; on macOS/Linux only the owner can read it. Do not sync this file to a shared cloud folder.

## Uninstall

```bash
ccl uninstall
```

This removes `~/.ccl` (binary, config, template, keys) and the PATH line the install script added. It does not touch `.claude/settings.local.json` in your projects: they keep working without `ccl`.
