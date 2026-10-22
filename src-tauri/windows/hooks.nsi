; Lets the Z21 reach TrainLink over UDP. Windows Firewall blocks unsolicited inbound UDP, and the Z21 sends its
; status to the port the app listens on. The rule is removed first so that reinstalling doesn't add a second copy

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="TrainLink Z21"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="TrainLink Z21" dir=in action=allow program="$INSTDIR\trainlink.exe" protocol=UDP enable=yes profile=private,domain'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="TrainLink Z21"'
!macroend
