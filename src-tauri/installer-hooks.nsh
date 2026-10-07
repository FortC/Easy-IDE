; EasyIDE NSIS 安装钩子：注册 Windows 右键菜单"用 EasyIDE 打开"（HKCU 当前用户，无需管理员）
; 卸载时自动清理

!macro NSIS_HOOK_POSTINSTALL
  ; 右键菜单直达项
  WriteRegStr HKCU "Software\Classes\EasyIDE.file" "" "EasyIDE 文档"
  WriteRegStr HKCU "Software\Classes\EasyIDE.file\DefaultIcon" "" "$INSTDIR\EasyIDE.exe,0"
  WriteRegStr HKCU "Software\Classes\EasyIDE.file\shell\open\command" "" '"$INSTDIR\EasyIDE.exe" "%1"'

  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.md\shell\EasyIDE" "" "用 EasyIDE 打开"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.md\shell\EasyIDE" "Icon" "$INSTDIR\EasyIDE.exe"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.md\shell\EasyIDE\command" "" '"$INSTDIR\EasyIDE.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\.md\OpenWithProgids\EasyIDE.file" "" ""

  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.markdown\shell\EasyIDE" "" "用 EasyIDE 打开"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.markdown\shell\EasyIDE" "Icon" "$INSTDIR\EasyIDE.exe"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.markdown\shell\EasyIDE\command" "" '"$INSTDIR\EasyIDE.exe" "%1"'

  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.canvas\shell\EasyIDE" "" "用 EasyIDE 打开"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.canvas\shell\EasyIDE" "Icon" "$INSTDIR\EasyIDE.exe"
  WriteRegStr HKCU "Software\Classes\SystemFileAssociations\.canvas\shell\EasyIDE\command" "" '"$INSTDIR\EasyIDE.exe" "%1"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.md\shell\EasyIDE"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.markdown\shell\EasyIDE"
  DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\.canvas\shell\EasyIDE"
  DeleteRegValue HKCU "Software\Classes\.md\OpenWithProgids" "EasyIDE.file"
  DeleteRegKey HKCU "Software\Classes\EasyIDE.file"
!macroend
