; DiagramCraft Windows installer (NSIS). Build:
;   makensis -DVERSION=<ver> packaging/installer.nsi
; from the repo root; expects target\release\diagramcraft.exe.

!define APPNAME "DiagramCraft"
; Local builds: makensis -DVERSION=x packaging/installer.nsi from the
; repo root defaults LICENSEDIR to the repo root.
!ifndef LICENSEDIR
  !define LICENSEDIR "."
!endif
; Binary directory passed in via -DBINDIR=... (NSIS resolves File
; paths relative to this script, not the invocation directory).
Name "${APPNAME}"
OutFile "diagramcraft_${VERSION}_windows_x64_setup.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel admin
Unicode true
SetCompressor /SOLID lzma

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "${APPNAME} (required)"
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${BINDIR}\diagramcraft.exe"
  CreateDirectory "$INSTDIR\licenses"
  SetOutPath "$INSTDIR\licenses"
  File /oname=LICENSE-MIT "${LICENSEDIR}\LICENSE-MIT"
  File /oname=LICENSE-APACHE "${LICENSEDIR}\LICENSE-APACHE"
  File /oname=LICENSE-Carlito "${LICENSEDIR}\crates\dc\assets\fonts\LICENSE-Carlito"
  SetOutPath "$INSTDIR"
  WriteRegStr HKLM "Software\${APPNAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
    "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
    "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
    "DisplayVersion" "${VERSION}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Start Menu shortcut"
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\diagramcraft.exe"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Desktop shortcut"
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\diagramcraft.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\licenses\LICENSE-MIT"
  Delete "$INSTDIR\licenses\LICENSE-APACHE"
  Delete "$INSTDIR\licenses\LICENSE-Carlito"
  RMDir "$INSTDIR\licenses"
  Delete "$INSTDIR\diagramcraft.exe"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
  DeleteRegKey HKLM "Software\${APPNAME}"
SectionEnd
