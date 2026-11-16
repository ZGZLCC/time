!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R8 HKLM "${MANUPRODUCTKEY}" ""
  ReadRegStr $R9 HKCU "${MANUPRODUCTKEY}" ""

  ; Older installers did not record the install mode. Follow their registered directory.
  ${If} $R8 != ""
  ${AndIf} $R9 == ""
    Call MultiUser.InstallMode.AllUsers
  ${ElseIf} $R9 != ""
  ${AndIf} $R8 == ""
    Call MultiUser.InstallMode.CurrentUser
  ${EndIf}

  Call RestorePreviousInstallLocation
  SetOutPath $INSTDIR
!macroend
