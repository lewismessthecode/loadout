# Local macOS delivery

After each completed Loadout app iteration, update the local macOS installation as part of delivery. This is a standing user instruction. Do not stop at a source commit or pull request.

1. Run the checks appropriate to the change.
2. Build the release app from the checkout containing the completed changes. Use `pnpm tauri build --target universal-apple-darwin --bundles app` for the universal macOS app.
3. Verify the new bundle before replacing `/Applications/Loadout.app`. Never install a debug bundle or a bundle configured with a fixture home.
4. Quit running Loadout processes and replace the installed app with the verified release bundle.
5. Launch `/Applications/Loadout.app` and verify that the installed process and interface work with the user's normal home directory.
6. After verification succeeds, remove superseded Loadout app bundles, including old build outputs, temporary replacement backups, and `Loadout Dev.app` test bundles. Keep only the current installed app. Inspect paths and bundle identifiers before deletion. Do not delete unrelated apps, skill backups, skills, preferences, or the usage database.
7. Report the installation path, verification result, and cleanup performed. If installation fails, restore the previous working app and report the failure.

Keep this instruction in the project root `AGENTS.md` so future iterations follow the same delivery steps.
