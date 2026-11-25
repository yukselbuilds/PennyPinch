# Windows desktop preview verification

Use this procedure on a test Windows desktop. Use a separate Windows user for the test. Use a clean OS without development tools for clean-install evidence. The procedure collects no model traffic.

## Check the desktop and ZIP

1. Download the ZIP for the selected test.
2. Open PowerShell in the PennyPinch source checkout.
3. Run the command for the selected test before you install PennyPinch.
4. Save the JSON line that the script prints.

For the current installer, use the `0.1.1-preview.6` ZIP. [Current preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.6)

```powershell
powershell.exe -NoProfile -File .\scripts\check-windows-preview.ps1 -Archive .\PennyPinch_0.1.1-preview.6_Windows_x64_preview.zip
```

For the signed update and rollback test, use the `0.1.1-preview.1` ZIP. [Signed update baseline](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.1)

```powershell
powershell.exe -NoProfile -File .\scripts\check-windows-preview.ps1 -Scenario SignedUpdate -Archive .\PennyPinch_0.1.1-preview.1_Windows_x64_preview.zip
```

The default target is Windows 11 x64. Use `-Target Windows10` on Windows 10 22H2 x64. A `ready` result means the desktop target and the ZIP's SHA-256 digest match. An empty `webview2_registry_versions` list means the script found no WebView2 Runtime registration. Keep that result as the preinstallation record. [WebView2 detection](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution#detect-if-a-webview2-runtime-is-already-installed)

If the status is `target_mismatch`, use the matching x64 Windows desktop. If it is `archive_mismatch`, select the ZIP for the current test. The preflight report shows only OS facts, release digests, and the local WebView2 version. Installation and rollback results require the steps below.

## Current installer

1. Extract the verified ZIP.
2. Compare the extracted setup file's SHA-256 digest with `installer_sha256` in the preflight JSON.
3. Run the setup file.
4. Open PennyPinch.
5. Select **Start** in **Overview**.
6. Wait for **Ready**.
7. Open **Engine options**.
8. Select **Stop engine**.
9. Wait for **Stopped**.
10. Exit PennyPinch from the system tray.
11. Run the same preflight command again.

Keep internet access available for setup if WebView2 is absent. Record the before and after WebView2 versions. Record each result for setup, the window, engine start, engine stop, and exit. [Installed upgrade evidence on Windows 10](../proofs/windows-preview4-installed-upgrade.json)

## Signed update and rollback

Use a separate test installation of `0.1.1-preview.1`. The signed Preview channel for this test serves `0.1.1-preview.2`. [Verified signed update](../proofs/pennypinch-windows10-signed-preview.json)

1. Extract the baseline ZIP.
2. Compare the extracted setup file's SHA-256 digest with `installer_sha256` in the preflight JSON.
3. Install the baseline.
4. Open PennyPinch.
5. Set **Theme** to **Dark** in **Settings**.
6. Open **App updates**.
7. Select **Preview**.
8. Select **Check for updates**.
9. Check that the offered version is `0.1.1-preview.2`.
10. Select the `Download and verify` button.
11. Select **Update and restart**.
12. Select **Continue**.
13. Check the installed version after restart.
14. Check the **Theme** setting.
15. Select **Restore previous version** in **App updates**.
16. Select **Continue**.
17. Check that `0.1.1-preview.1` reopens.
18. Check the **Theme** setting again.

Record the version before update, after update, and after rollback. Record whether the saved theme matched at each restart. Stop the test if the channel offers a different version. [Installed update and rollback evidence on Windows 10](../proofs/pennypinch-windows10-installed-update-and-release.json)

## Report the result

Share the preflight JSON with the outcomes of the steps you ran. Use `pass`, `fail`, or `not_tested` for each step. Report whether the desktop was a clean installation. Keep local paths, account details, and chat contents out of the report.
