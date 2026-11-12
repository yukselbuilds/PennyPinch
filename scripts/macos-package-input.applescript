on run arguments
    set actionName to item 1 of arguments
    try
        with timeout of 70 seconds
            tell application "System Events"
                set pickerApp to first application process whose bundle identifier is "dev.headroom.desktop.h048c5"
                set frontmost of pickerApp to true
            end tell
            if actionName is "open" then
                my waitControl(pickerApp, "Başlat", {"AXButton"}, false)
            else if actionName is "start" then
                my waitControl(pickerApp, "Başlat", {"AXButton"}, true)
                my waitControl(pickerApp, "Duraklat", {"AXButton"}, false)
            else if actionName is "stop" then
                my waitControl(pickerApp, "Motor seçenekleri", {"AXButton", "AXDisclosureTriangle"}, true)
                my waitControl(pickerApp, "Motoru durdur", {"AXButton"}, true)
                my waitControl(pickerApp, "Başlat", {"AXButton"}, false)
            else if actionName is "exit" then
                my waitControl(pickerApp, "Headroom’dan çık", {"AXButton"}, true)
                my waitControl(pickerApp, "Çıkışı onayla", {"AXButton"}, true)
            else
                error number 1801
            end if
        end timeout
        log "headroom-package-ui=" & actionName & "_pass"
    on error number failureCode
        log "headroom-package-ui=failed code=" & failureCode
        error number failureCode
    end try
end run

on waitControl(pickerApp, labelText, acceptedRoles, activateControl)
    set observedControls to 0
    set targetSeen to false
    tell application "System Events"
        repeat 120 times
            if not (exists pickerApp) then error number 1802
            if exists window "Headroom" of pickerApp then
                set allControls to entire contents of window "Headroom" of pickerApp
                set observedControls to count of allControls
                if (count of allControls) > 1200 then error number 1803
                repeat with controlRef in allControls
                    set controlItem to contents of controlRef
                    if (role of controlItem) is in acceptedRoles then
                        set labelMatches to false
                        try
                            set labelMatches to (name of controlItem) is labelText
                        end try
                        if not labelMatches then
                            try
                                set labelMatches to (description of controlItem) is labelText
                            end try
                        end if
                        if labelMatches then
                            set targetSeen to true
                            if enabled of controlItem then
                                if activateControl then click controlItem
                                return
                            end if
                        end if
                    end if
                end repeat
            end if
            delay 0.2
        end repeat
        log "headroom-package-ui=control_timeout label=" & labelText & " controls=" & observedControls & " target_seen=" & targetSeen
        error number 1804
    end tell
end waitControl
