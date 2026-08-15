; ScoreLeap NSIS 安装钩子（Issue #61）
; 在开始菜单补充「卸载」入口；卸载时同步删除。
; 被 tauri.conf.json bundle.windows.nsis.installerHooks 引用。
; 注意：模板宏（${APPNAME} 等）在 hook 上下文不可用，此处自行定义。
; 应用名需与 tauri.conf.json 的 productName 保持一致。

!define SL_UNINSTALL_LINK "卸载 ScoreLeap"

!macro NSIS_HOOK_POSTINSTALL
  ; 安装完成后：创建开始菜单卸载快捷方式
  CreateShortcut "$SMPROGRAMS\${SL_UNINSTALL_LINK}.lnk" "$INSTDIR\uninstall.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 卸载前：删除开始菜单卸载快捷方式
  Delete "$SMPROGRAMS\${SL_UNINSTALL_LINK}.lnk"
!macroend
