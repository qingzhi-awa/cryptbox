!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_ICONQUESTION "是否同时删除本地密码库数据？$\n$\n选「是」将删除所有本地数据（不可恢复），选「否」则保留数据。" IDYES delete_data IDNO skip_delete
  delete_data:
    RMDir /r "$APPDATA\CryPtBox"
  skip_delete:
!macroend
