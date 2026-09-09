; CryPtBox Windows 安装脚本（NSIS 3）
; 用法：makensis installer.nsi
; 产物：CryPtBox-setup.exe

Unicode true
!include "MUI2.nsh"

!define APPNAME "CryPtBox"
!define APPEXE "CryPtBox.exe"
!define APPVERSION "0.2.0"

Name "${APPNAME}"
OutFile "CryPtBox-setup.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "安装"
  SetOutPath "$INSTDIR"
  File "build\bin\${APPEXE}"
  File "build\import-template.csv"

  ; 开始菜单
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\${APPEXE}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\卸载 CryPtBox.lnk" "$INSTDIR\Uninstall.exe"

  ; 桌面快捷方式
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\${APPEXE}"

  ; 卸载器与注册表卸载信息
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayVersion" "${APPVERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "Publisher" "CryPtBox"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "UninstallString" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  ; 仅删除程序文件，不删除用户密码库（%APPDATA%\CryPtBox），避免误删数据。
  Delete "$INSTDIR\${APPEXE}"
  Delete "$INSTDIR\import-template.csv"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\卸载 CryPtBox.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"

  Delete "$DESKTOP\${APPNAME}.lnk"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
SectionEnd
