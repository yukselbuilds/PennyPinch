on run arguments
    set actionName to item 1 of arguments
    set targetPid to (item 2 of arguments) as integer
    with timeout of 15 seconds
        tell application "System Events"
            set matches to every application process whose unix id is targetPid
            if (count of matches) is not 1 then error number 1901
            set ownedApp to item 1 of matches
            if (bundle identifier of ownedApp) is not "com.openai.codex" then error number 1902
            if actionName is "quit" then
                set frontmost of ownedApp to true
                if not (frontmost of ownedApp) then error number 1903
                keystroke "q" using command down
                return "{}"
            end if
            if actionName is not "observe" then error number 1904
            set windowCount to count of windows of ownedApp
            set controlCount to 0
            set signIn to false
            set apiKey to false
            repeat with ownedWindow in windows of ownedApp
                try
                    set controls to entire contents of ownedWindow
                    set controlCount to controlCount + (count of controls)
                    if controlCount > 2000 then error number 1905
                    repeat with controlRef in controls
                        set controlItem to contents of controlRef
                        if role of controlItem is "AXButton" then
                            set labelText to name of controlItem
                            if labelText is in {"Sign in", "Sign in with ChatGPT", "Log in", "Log in with ChatGPT", "Continue with ChatGPT"} then set signIn to true
                            if labelText is in {"Use an API key", "Sign in with API key", "Continue with API key"} then set apiKey to true
                        end if
                    end repeat
                on error number failureCode
                    if failureCode is not -1728 and failureCode is not -1719 then error number failureCode
                end try
            end repeat
            return "{\"windows\":" & windowCount & ",\"controls\":" & controlCount & ",\"sign_in_button\":" & my jsonBoolean(signIn) & ",\"api_key_button\":" & my jsonBoolean(apiKey) & "}"
        end tell
    end timeout
end run

on jsonBoolean(flag)
    if flag then return "true"
    return "false"
end jsonBoolean
