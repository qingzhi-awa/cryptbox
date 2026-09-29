!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONQUESTION "Do you also want to delete local password data?$\n$\nChoose Yes to delete all local data (irreversible), or No to keep it." IDYES delete_data IDNO skip_delete
  delete_data:
    RMDir /r "$APPDATA\CryPtBox"
  skip_delete:
!macroend
