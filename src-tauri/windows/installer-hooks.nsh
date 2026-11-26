; Ganchos del instalador NSIS de Nebula Reader.
;
; Los PDF NO se asocian como formato predeterminado: la mayoría de usuarios ya
; tiene un lector de PDF y no debe cambiarse sin su permiso. Nebula Reader solo
; se añade a «Abrir con» (OpenWithProgids). CBZ, CBR, CB7 y EPUB sí se asocian
; mediante `bundle.fileAssociations` en tauri.conf.json.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\NebulaReader.Pdf" "" "Documento PDF"
  WriteRegStr SHCTX "Software\Classes\NebulaReader.Pdf\DefaultIcon" "" "$INSTDIR\nebula-reader.exe,0"
  WriteRegStr SHCTX "Software\Classes\NebulaReader.Pdf\shell\open\command" "" '"$INSTDIR\nebula-reader.exe" "%1"'
  WriteRegStr SHCTX "Software\Classes\.pdf\OpenWithProgids" "NebulaReader.Pdf" ""
  ; Avisa al Explorador de que han cambiado las asociaciones.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue SHCTX "Software\Classes\.pdf\OpenWithProgids" "NebulaReader.Pdf"
  DeleteRegKey SHCTX "Software\Classes\NebulaReader.Pdf"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
