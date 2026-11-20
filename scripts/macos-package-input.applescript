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
                my waitControl(pickerApp, "PennyPinch’ten çık", {"AXButton"}, true)
                my waitControl(pickerApp, "Çıkışı onayla", {"AXButton"}, true)
            else if actionName starts with "export_" then
                my waitControl(pickerApp, "Dışa aktar", {"AXButton", "AXDisclosureTriangle"}, true)
                if actionName is "export_statistics_csv" then
                    set reportLabel to "İstatistik · CSV"
                else if actionName is "export_diagnostics_json" then
                    set reportLabel to "Tanı raporu · JSON"
                else if actionName is in {"export_cancel", "export_statistics_json"} then
                    set reportLabel to "İstatistik · JSON"
                else
                    error number 1810
                end if
                my waitControl(pickerApp, reportLabel, {"AXButton"}, true)
                my saveReport(pickerApp, actionName, arguments)
                if actionName is "export_cancel" then
                    my waitControl(pickerApp, "Kaydetme iptal edildi.", {"AXStaticText"}, false)
                else
                    my waitControl(pickerApp, "Rapor yeni dosyaya kaydedildi.", {"AXStaticText"}, false)
                end if
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
    set staleReads to 0
    tell application "System Events"
        repeat 120 times
            if not (exists pickerApp) then error number 1802
            if exists window "PennyPinch" of pickerApp then
                set targetControl to missing value
                try
                    set allControls to entire contents of window "PennyPinch" of pickerApp
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
                            if not labelMatches and (role of controlItem) is "AXStaticText" then
                                try
                                    set labelMatches to (value of controlItem) starts with labelText
                                end try
                            end if
                            if labelMatches then
                                set targetSeen to true
                                if enabled of controlItem then
                                    set targetControl to controlItem
                                    exit repeat
                                end if
                            end if
                        end if
                    end repeat
                on error number failureCode
                    -- A WebView render can invalidate a read; clicks are never retried here.
                    if failureCode is not -1728 and failureCode is not -1719 then error number failureCode
                    set staleReads to staleReads + 1
                    set targetControl to missing value
                end try
                if targetControl is not missing value then
                    if staleReads > 0 then log "headroom-package-ui=read_refreshes count=" & staleReads
                    if activateControl then
                        try
                            click targetControl
                        on error number failureCode
                            log "headroom-package-ui=click_failed label=" & labelText & " code=" & failureCode
                            error number failureCode
                        end try
                    end if
                    return
                end if
            end if
            delay 0.2
        end repeat
        log "headroom-package-ui=control_timeout label=" & labelText & " controls=" & observedControls & " target_seen=" & targetSeen
        error number 1804
    end tell
end waitControl

on saveReport(pickerApp, actionName, arguments)
    tell application "System Events"
        repeat 100 times
            if exists window "Raporu kaydet" of pickerApp then exit repeat
            delay 0.15
        end repeat
        if not (exists window "Raporu kaydet" of pickerApp) then error number 1811
        set saveWindow to window "Raporu kaydet" of pickerApp
        if not (frontmost of pickerApp) then error number 1812
        if actionName is "export_cancel" then
            click button "Cancel" of saveWindow
            return
        end if
        key code 5 using {command down, shift down}
        repeat 40 times
            if exists sheet 1 of saveWindow then exit repeat
            delay 0.15
        end repeat
        if not (exists sheet 1 of saveWindow) then error number 1813
        set goSheet to sheet 1 of saveWindow
        set pathInput to missing value
        set controls to entire contents of goSheet
        if (count of controls) > 80 then error number 1814
        repeat with controlRef in controls
            set controlItem to contents of controlRef
            if (role of controlItem) is in {"AXTextField", "AXComboBox"} then
                set pathInput to controlItem
                exit repeat
            end if
        end repeat
        if pathInput is missing value then error number 1815
        set value of pathInput to item 2 of arguments
        if (value of pathInput) is not item 2 of arguments then error number 1816
        if exists button "Go" of goSheet then
            click button "Go" of goSheet
        else
            key code 36
        end if
        repeat 40 times
            if not (exists sheet 1 of saveWindow) then exit repeat
            delay 0.15
        end repeat
        if exists sheet 1 of saveWindow then error number 1817
        set nameInput to missing value
        set controls to entire contents of saveWindow
        if (count of controls) > 400 then error number 1818
        repeat with controlRef in controls
            set controlItem to contents of controlRef
            if (role of controlItem) is "AXTextField" then
                try
                    if (value of controlItem) starts with "headroom-" then
                        set nameInput to controlItem
                        exit repeat
                    end if
                end try
            end if
        end repeat
        if nameInput is missing value then error number 1819
        set value of nameInput to item 3 of arguments
        if (value of nameInput) is not item 3 of arguments then error number 1820
        if not (frontmost of pickerApp) then error number 1812
        if not (enabled of button "Save" of saveWindow) then error number 1821
        click button "Save" of saveWindow
    end tell
end saveReport
