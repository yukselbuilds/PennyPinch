# Project Ponytail

26 October 2026. Three instruction skills support development in this repository.

| Skill | Usage |
| --- | --- |
| `.agents/skills/ponytail/SKILL.md` | Use `full` for implementation and fixes. Make the smallest change that fully solves the task. |
| `.agents/skills/ponytail-review/SKILL.md` | Review the changed code, relevant callers and tests before acceptance. |
| `.agents/skills/ponytail-audit/SKILL.md` | Produce a read-only audit of existing code in the requested scope. |

The source is `DietrichGebert/ponytail` at pinned commit `09c434ab73ca43761c5d8b0c5d341926c2fba710`. The repository preserves the upstream text and MIT license. See the [source manifest](ponytail-source.json) for hashes and the [license](ponytail-LICENSE.txt) for the terms. Local installation evidence lives under `work/proofs/dev-tools/`. The setup consists of instruction files.

## Using the skills in this repository

The lead agent and the single worker use the same files. Apply `AGENTS.md` together with the skills. Use the `full` level, preserve the selected model and worker settings, and base acceptance on actual evidence. Write repository reports in English and communicate with the repository owner in Turkish. Treat a `Ship` verdict as a review outcome. Publishing and pushing use the user's existing authorization. Read the skill when needed even when discovery has already found it.

Use `ponytail` for new coding work, `ponytail-review` for change acceptance and `ponytail-audit` for a requested historical code audit. Keep reviews focused on the changed or requested area. Preserve required verification, data-loss prevention, security, accessibility and rollback checks.

The repository uses project-local instructions. Report savings percentages only from actual token measurements. See [WORKFLOW.md](WORKFLOW.md) for the shared workflow and acceptance criteria.

## Discovery and verification

OpenAI's local skill documentation describes the `.agents/skills` path and automatic discovery. Verify installation by checking the path, metadata, pinned source hash and file readability. Read the files directly when the current chat has an older skill catalog and use discovery once the catalog reloads. The installation check requires no model or provider request or restart of another chat.

[OpenAI local skills documentation](https://learn.chatgpt.com/docs/build-skills) · [Pinned Ponytail source](https://github.com/DietrichGebert/ponytail/tree/09c434ab73ca43761c5d8b0c5d341926c2fba710)
