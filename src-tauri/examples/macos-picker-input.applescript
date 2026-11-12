on run arguments
    set pickerPid to (item 1 of arguments) as integer
    set fixturePath to item 2 of arguments
    with timeout of 6 seconds
        try
            tell application "System Events"
                set pickerApp to first application process whose unix id is pickerPid
                if not (frontmost of pickerApp) then return "not_frontmost"
                if (count of windows of pickerApp) is not 1 then return "panel_unavailable"
                key code 5 using {command down, shift down}

                set pathInput to missing value
                repeat 16 times
                    delay 0.15
                    if not (frontmost of pickerApp) then return "focus_lost"
                    set focusedItem to value of attribute "AXFocusedUIElement" of pickerApp
                    if focusedItem is not missing value then
                        if (role of focusedItem) is in {"AXTextField", "AXComboBox"} then
                            set pathInput to focusedItem
                            exit repeat
                        end if
                    end if
                end repeat
                if pathInput is missing value then return "path_input_unavailable"

                key code 9 using {command down}
                repeat 12 times
                    if (value of pathInput) is fixturePath then exit repeat
                    delay 0.15
                end repeat
                if (value of pathInput) is not fixturePath then return "paste_unobserved"
                if not (frontmost of pickerApp) then return "focus_lost"
                key code 36

                repeat 16 times
                    delay 0.15
                    if (count of windows of pickerApp) is 0 then return "panel_closed_after_return"
                    if not (frontmost of pickerApp) then return "focus_lost"
                    set panelWindow to first window of pickerApp
                    set confirmButton to value of attribute "AXDefaultButton" of panelWindow
                    if confirmButton is not missing value then
                        if (name of confirmButton) is in {"Open", "Choose"} and (enabled of confirmButton) then
                            click confirmButton
                            return "clicked_confirmation"
                        end if
                    end if
                end repeat
                return "confirmation_unavailable"
            end tell
        on error number errorNumber
            return "system_events_error_" & (errorNumber as text)
        end try
    end timeout
end run
