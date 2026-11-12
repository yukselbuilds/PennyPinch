on run arguments
    set fixtureRoot to item 1 of arguments
    try
        my chooseFixture("Select Codex command", fixtureRoot & "/CLI 工具/codex")
        log "headroom-picker-input-driver=file_confirmed"
        my chooseFixture("Select Claude Code project folder", fixtureRoot & "/Claude 项目")
        log "headroom-picker-input-driver=folder_confirmed"
    on error number failureCode
        log "headroom-picker-input-driver=failed code=" & failureCode
        error number failureCode
    end try
end run

on chooseFixture(panelTitle, fixturePath)
    with timeout of 20 seconds
        tell application "System Events"
            set pickerApp to missing value
            repeat 60 times
                if exists (first application process whose bundle identifier is "dev.headroom.picker-proof") then
                    set pickerApp to first application process whose bundle identifier is "dev.headroom.picker-proof"
                    if exists window panelTitle of pickerApp then exit repeat
                end if
                delay 0.15
            end repeat
            if pickerApp is missing value then error number 1701
            if not (exists window panelTitle of pickerApp) then error number 1701
            set frontmost of pickerApp to true
            repeat 20 times
                if frontmost of pickerApp then exit repeat
                delay 0.15
            end repeat
            if not (frontmost of pickerApp) then error number 1702
            log "headroom-picker-input-driver=owned_panel_frontmost"
            key code 5 using {command down, shift down}
            repeat 20 times
                if exists sheet 1 of window panelTitle of pickerApp then exit repeat
                delay 0.15
            end repeat
            if not (exists sheet 1 of window panelTitle of pickerApp) then error number 1703
        end tell
        set the clipboard to fixturePath
        tell application "System Events"
            if not (frontmost of pickerApp) then error number 1704
            key code 9 using {command down}
            delay 0.4
            key code 36
            repeat 20 times
                if not (exists sheet 1 of window panelTitle of pickerApp) then exit repeat
                delay 0.15
            end repeat
            if exists sheet 1 of window panelTitle of pickerApp then error number 1705
            if not (exists window panelTitle of pickerApp) then return
            if not (frontmost of pickerApp) then error number 1706
            key code 36
            repeat 20 times
                if not (exists window panelTitle of pickerApp) then return
                delay 0.15
            end repeat
            error number 1707
        end tell
    end timeout
end chooseFixture
