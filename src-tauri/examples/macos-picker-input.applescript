on run arguments
    set pickerPid to (item 1 of arguments) as integer
    set fixtureName to item 2 of arguments
    with timeout of 6 seconds
        try
            tell application "System Events"
                set pickerApp to first application process whose unix id is pickerPid
                if (count of windows of pickerApp) is not 1 then return "panel_unavailable"
                set panelWindow to first window of pickerApp
                set controls to entire contents of panelWindow
                if (count of controls) > 300 then return "fixture_controls_limit"
                set fixtureFound to false
                set fixtureSelected to false
                repeat with controlRef in controls
                    set controlItem to contents of controlRef
                    if (role of controlItem) is in {"AXRow", "AXCell", "AXTextField", "AXStaticText", "AXImage"} then
                        if my isFixture(controlItem, fixtureName) then
                            set fixtureFound to true
                            set fixtureSelected to my selectFixture(controlItem)
                            if fixtureSelected then exit repeat
                        end if
                    end if
                end repeat
                if not fixtureFound then return "fixture_row_missing"
                if not fixtureSelected then return "fixture_row_not_selectable"
                repeat with confirmName in {"Open", "Choose"}
                    set labelText to contents of confirmName
                    if exists button labelText of panelWindow then
                        set confirmButton to button labelText of panelWindow
                        if enabled of confirmButton then
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

on isFixture(controlItem, fixtureName)
    tell application "System Events"
        repeat with labelAttribute in {"AXFilename", "AXTitle", "AXValue", "AXDescription"}
            try
                if (value of attribute (contents of labelAttribute) of controlItem) is fixtureName then return true
            end try
        end repeat
    end tell
    return false
end isFixture

on selectFixture(controlItem)
    tell application "System Events"
        repeat 5 times
            if controlItem is missing value then return false
            set roleName to role of controlItem
            if roleName is "AXWindow" then return false
            if roleName is in {"AXRow", "AXCell", "AXImage"} then
                try
                    set value of attribute "AXSelected" of controlItem to true
                    if (value of attribute "AXSelected" of controlItem) is true then return true
                end try
            end if
            set parentItem to value of attribute "AXParent" of controlItem
            if roleName is "AXRow" and parentItem is not missing value then
                try
                    set value of attribute "AXSelectedRows" of parentItem to {controlItem}
                    if (value of attribute "AXSelected" of controlItem) is true then return true
                end try
            end if
            set controlItem to parentItem
        end repeat
    end tell
    return false
end selectFixture
