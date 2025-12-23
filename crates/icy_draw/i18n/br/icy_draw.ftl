font-editor-table = Tabela de caracteres 0-{ $length }:

unsaved-title=Sem título

# CLI (clap)
app-about = Icy Draw — create ANSI/ASCII art, edit fonts, and collaborate in real time
arg-path-help = File to open on startup
arg-mcp-port-help = Start an MCP server on the given port (e.g. --mcp-port 8080)
cmd-host-about = Host a real-time collaboration session (Moebius-compatible)
arg-host-port-help = Port to listen on (default: 8000)
arg-host-bind-help = Bind address (default: 0.0.0.0)
arg-host-password-help = Session password (optional)
arg-host-max-users-help = Maximum number of users (0 = unlimited)
arg-host-file-help = File to host (optional; starts with an empty 80x25 canvas if omitted)
arg-host-backup-folder-help = Folder for autosave backups (enables autosave if set)
arg-host-interval-help = Autosave interval in minutes (default: 60)

# Server banner
server-title = Icy Draw Collaboration Server
server-bind-address = Bind Address
server-password = Password
server-document = Document
server-max-users = Max Users
server-connect-with = Connect with
server-stop-hint = Press Ctrl+C to stop the server
server-none = (none)
server-unlimited = unlimited

menu-file=Arquivo
menu-new=Novo…
menu-open=Abrir…
menu-open_recent=Abrir recente
menu-open_recent_clear=Limpar
menu-no_recent_files=Nenhum arquivo recente
menu-clear_recent_files=Limpar lista
menu-save=Salvar
menu-edit-sauce=Editar informações SAUCE…
menu-9px-font=Fonte 9px
menu-aspect-ratio=Proporção de aspecto legado
menu-set-canvas-size=Definir tamanho da tela…
menu-file-settings=File Settings…

file-settings-dialog-title=File Settings
file-settings-canvas-size=Canvas Size
file-settings-format=Format
file-settings-sauce=SAUCE
file-settings-title=Title
file-settings-author=Author
file-settings-group=Group
file-settings-ice=Ice
file-settings-legacy-ar=Legacy Aspect Ratio
file-settings-9px-font=9px Font
file-settings-comments-button=Comments…
file-settings-settings-button=Settings…
file-settings-comments-title=SAUCE Comments
file-settings-comments-info=Max 255 lines, 64 characters per line
menu-close=Fechar
menu-save-as=Salvar como…
menu-export=Exportar…
menu-edit-font-outline=Contorno da fonte…
menu-show_settings=Configurações…

menu-edit=Editar
menu-undo=Desfazer
menu-redo=Refazer
menu-undo-op=Desfazer: { $op }
menu-redo-op=Refazer: { $op }

menu-cut=Cortar
menu-copy=Copiar
menu-paste=Colar
menu-delete=Excluir
menu-rename=Renomear
menu-paste-as=Colar como
menu-paste-as-new-image=Nova imagem
menu-paste-as-brush=Pincel
menu-erase=Apagar
menu-flipx=Inverter X
menu-flipy=Inverter Y
menu-justifyleft=Justificar à esquerda
menu-justifyright=Justificar à direita
menu-justifycenter=Centralizar
menu-crop=Cortar
menu-justify_line_center=Centralizar linha
menu-justify_line_left=Justificar linha à esquerda
menu-justify_line_right=Justificar linha à direita
menu-insert_row=Inserir linha
menu-delete_row=Excluir linha
menu-insert_colum=Inserir coluna
menu-delete_colum=Excluir coluna
menu-erase_row=Apagar linha
menu-erase_row_to_start=Apagar linha até o início
menu-erase_row_to_end=Apagar linha até o fim
menu-erase_column=Apagar coluna
menu-erase_column_to_start=Apagar coluna até o início
menu-erase_column_to_end=Apagar coluna até o fim
menu-scroll_area_up=Rolar área para cima
menu-scroll_area_down=Rolar área para baixo
menu-scroll_area_left=Rolar área para a esquerda
menu-scroll_area_right=Rolar área para a direita
menu-mirror_mode=Modo espelho
menu-area_operations=Área

menu-selection=Seleção
menu-select-all=Selecionar tudo
menu-select_nothing=Desmarcar
menu-inverse_selection=Inverter

menu-colors=Cores
menu-ice-mode=Modo Ice
menu-ice-mode-unrestricted=Sem restrições
menu-ice-mode-blink=Piscando
menu-ice-mode-ice=Ice
menu-palette-mode=Modo de paleta
menu-palette-mode-unrestricted=Sem restrições
menu-palette-mode-dos=Dos 16
menu-palette-mode-free=Livre 16
menu-palette-mode-free8=Livre 8

menu-select_palette=Selecionar paleta
menu-next_fg_color=Próxima cor de primeiro plano
menu-next_bg_color=Próxima cor de fundo
menu-prev_fg_color=Cor de primeiro plano anterior
menu-prev_bg_color=Cor de fundo anterior

menu-view=Visualizar
menu-reference-image=Abrir imagem de referência…
menu-toggle-reference-image=Alternar imagem de referência
menu-clear-reference-image=Limpar
menu-toggle_fullscreen=Tela cheia
menu-zoom=Zoom
menu-zoom_reset=Reverter zoom
menu-zoom_in=Aumentar zoom
menu-zoom_out=Diminuir zoom
menu-guides=Guias
menu-raster=Grade
menu-zoom-fit_size=Ajustar tamanho
menu-show_layer_borders=Mostrar bordas da camada
menu-show_line_numbers=Mostrar números de linha
menu-toggle_guide=Alternar guias
menu-toggle_raster=Alternar grade
menu-toggle_left_pane=Alternar painel esquerdo
menu-toggle_right_pane=Alternar painel direito

menu-pick_attribute_under_caret=Selecionar atributo
menu-default_color=Cor padrão
menu-toggle_color=Alternar cor de primeiro plano/fundo

menu-fonts=Fontes
menu-font-mode=Modo de fonte
menu-font-mode-unrestricted=Sem restrições
menu-font-mode-sauce=Sauce
menu-font-mode-single=Única
menu-font-mode-dual=Dupla
menu-open_font_selector=Selecionar fonte…
menu-add_fonts=Adicionar fontes…
menu-open_font_manager=Editar fontes do buffer…
menu-open_font_directoy=Abrir diretório de fontes…
menu-open_palettes_directoy=Abrir diretório de paletas…

menu-help=Ajuda
menu-discuss=Discutir
menu-open_log_file=Abrir arquivo de log
menu-report-bug=Relatar bug
menu-about=Sobre…
menu-plugins=Plugins
menu-open_plugin_directory=Abrir diretório de plugins…

menu-upgrade_version=Atualizar para { $version }

tool-fg=Fg
tool-bg=Bg
tool-solid=Sólido
tool-character=Caractere
tool-shade=Sombra
tool-colorize=Colorir
tool-size-label=Tamanho
tool-full-block=Bloco
tool-half-block=Meio bloco
tool-outline=Contorno
tool-custom-brush=Pincel personalizado

tool-select-label=Modo de seleção
tool-select-normal=Retângulo
tool-select-character=Caractere
tool-select-attribute=Atributo
tool-select-foreground=Primeiro plano
tool-select-background=Fundo
tool-select-description=Segure shift para adicionar à seleção. Control/Cmd para remover.

tool-fill-exact_match_label=Correspondência exata
tool-flip_horizontal=Horizontal
tool-flip_vertical=Vertical

tool-paint_brush_name=Pincel de pintura
tool-paint_brush_tooltip=Pintar traços usando um pincel
tool-click_name=Entrada de texto
tool-click_tooltip=Inserir texto e seleções retangulares
tool-ellipse_name=Elipse
tool-ellipse_tooltip=Desenhar elipse
tool-filled_ellipse_name=Elipse preenchida
tool-filled_ellipse_tooltip=Desenhar elipse preenchida
tool-rectangle_name=Retângulo
tool-rectangle_tooltip=Desenhar retângulo
tool-filled_rectangle_name=Retângulo preenchido
tool-filled_rectangle_tooltip=Desenhar retângulo preenchido
tool-eraser_name=Borracha
tool-eraser_tooltip=Apagar para o fundo usando um pincel
tool-fill_name=Preencher
tool-fill_tooltip=Preencher área com cor ou caractere
tool-flip_name=Inversor
tool-flip_tooltip=Inverter blocos verticais ou horizontais
tool-tdf_name=Fontes The Draw
tool-tdf_tooltip=Entrada de texto usando fontes The Draw
tool-line_name=Desenhar linha
tool-line_tooltip=Desenhar linhas
tool-move_layer_name=Mover camada
tool-move_layer_tooltip=Mover camadas
tool-pencil_name=Lápis
tool-pencil_tooltip=Pintar traços usando um lápis
tool-pipette_name=Conta-gotas
tool-pipette_tooltip=Selecionar uma cor
tool-select_name=Ferramenta de seleção
tool-select_tooltip=Seleções múltiplas e não retangulares
tool-tag_name=Ferramenta de tag
tool-tag_tooltip=Tags são usadas para expandir strings na saída
tool-tag_show=Show Tags
tool-tag_edit_button={ menu-edit }

toolbar-new=Novo

new-file-title=Novo arquivo
new-file-width=Largura
new-file-height=Altura
new-file-ok=Ok
new-file-cancel=Cancelar
new-file-create=Criar

edit-sauce-title=Editar informações SAUCE
edit-sauce-title-label=Título
edit-sauce-title-label-length=(35 caracteres)
edit-sauce-author-label=Autor
edit-sauce-author-label-length=(20 caracteres)
edit-sauce-group-label=Grupo
edit-sauce-group-label-length=(20 caracteres)
edit-sauce-comments-label=Comentários (limite de 64 caracteres por linha)
edit-sauce-letter-spacing=Usar modo 9px
edit-sauce-aspect-ratio=Simular proporção clássica

edit-canvas-size-title=Definir tamanho da tela
edit-canvas-size-width-label=Largura
edit-canvas-size-height-label=Altura
edit-canvas-size-resize=Redimensionar
edit-canvas-size-resize_layers-label=Redimensionar camadas

toolbar-size = { $colums ->
     [1] 1 Coluna
*[other] { $colums } Colunas
} x { $rows ->
     [1] 1 Linha
*[other] { $rows } Linhas
}

toolbar-position = Ln { $line }, Col { $column }
toolbar-layer_offset = Deslocamento da camada: { $line }x{ $column }
add_layer_tooltip = Adicionar nova camada
move_layer_up_tooltip = Mover camada para cima
move_layer_down_tooltip = Mover camada para baixo
delete_layer_tooltip = Excluir camada
anchor_layer_tooltip = Ancorar camada

glyph-char-label=Caractere
glyph-font-label=Fonte

color-is_blinking=Piscando

export-title=Exportar
export-button-title=Exportar
export-file-label=Nome do arquivo:
export-video-preparation-label=Preparação de vídeo:
export-video-preparation-None=Nenhum
export-video-preparation-Clear=Limpar tela
export-video-preparation-Home=Cursor inicial
export-utf8-output-label=Formato de terminal moderno (utf8)
export-save-sauce-label=Salvar informações SAUCE
export-compression-label=Comprimir saída
export-limit-output-line-length-label=Limitar comprimento da linha de saída
export-maximum_line_length=Comprimento máximo da linha
export-use_repeat_sequences=Usar sequências de repetição CSI Pn b
export-save_full_line_length=Salvar espaços em branco finais
export-format-label=Formato:
export-path-label=Caminho:

select-character-title=Selecionar caractere

select-outline-style-title=Tipo de estilo de contorno de fonte

about-dialog-title=Sobre o Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Icy Draw é uma ferramenta para criar arte ANSI e ASCII.
    É escrito em Rust e usa a biblioteca EGUI.

    Icy Draw é um software livre, licenciado sob a licença Apache 2.
    O código-fonte está disponível em www.github.com/mkrueger/icy_draw
about-dialog-created_by =
    Criado por { $authors }
    Ajuda e testes: NuSkooler, Grymmjack

edit-layer-dialog-title=Propriedades da camada
edit-layer-dialog-name-label=Nome
edit-layer-dialog-is-visible-checkbox=Visível
edit-layer-dialog-is-edit-locked-checkbox=Edição bloqueada
edit-layer-dialog-is-position-locked-checkbox=Posição bloqueada
edit-layer-dialog-is-x-offset-label=Deslocamento X
edit-layer-dialog-is-y-offset-label=Deslocamento Y
edit-layer-dialog-has-alpha-checkbox=Tem alpha
edit-layer-dialog-is-alpha-locked-checkbox=Alpha bloqueado

error-load-file=Erro ao carregar arquivo: { $error }

select-font-dialog-title=Selecionar fonte ({ $fontcount} disponíveis)
add-font-dialog-title=Adicionar fonte ({ $fontcount} disponíveis)
select-font-dialog-select=Selecionar
add-font-dialog-select=Adicionar
select-font-dialog-filter-text=Filtrar fontes
select-font-dialog-no-fonts=Nenhuma fonte corresponde ao filtro
select-font-dialog-no-fonts-installed=Nenhuma fonte instalada
select-font-dialog-color-font=COR
select-font-dialog-block-font=BLOCO
select-font-dialog-outline-font=CONTORNO
select-font-dialog-figlet-font=FIGLET
select-font-dialog-preview-text=OLÁ
select-font-dialog-edit-button=Editar fonte…

layer_tool_title=Camadas
layer_tool_menu_layer_properties=Propriedades da camada
layer_tool_menu_resize_layer=Redimensionar camada
layer_tool_menu_new_layer=Nova camada
layer_tool_menu_duplicate_layer=Duplicar camada
layer_tool_menu_merge_layer=Mesclar camada
layer_tool_menu_delete_layer=Excluir camada
layer_tool_menu_clear_layer=Limpar camada

channel_tool_title=Canais
channel_tool_fg=Primeiro plano
channel_tool_bg=Fundo

font_tool_select_outline_button=Contorno
font_tool_current_font_label=Fonte TDF atual
font_tool_no_font=<nenhuma>
font_tool_no_fonts_label=
    Nenhuma fonte tdf encontrada.
    Instale novas fontes no diretório de fontes
font_tool_open_directory_button=Abrir diretório de fontes

pipette_tool_char_code=Código { $code }
pipette_tool_foreground=Primeiro plano { $fg }
pipette_tool_background=Fundo { $bg }
pipette_tool_keys=
    Segure shift para selecionar
    cor de primeiro plano

    Segure control para selecionar
    cor de fundo

char_table_tool_title=Tabela de caracteres
minimap_tool_title=Pré-visualização

no_document_selected=Nenhum documento selecionado

undo-draw-ellipse=Desenhar elipse
undo-draw-rectangle=Desenhar retângulo
undo-paint-brush=Pincel
undo-pencil=Lápis
undo-eraser=Borracha
undo-bucket-fill=Preenchimento
undo-line=Linha
undo-cut=Cortar
undo-paste-glyph=Colar glifo
undo-bitfont-flip-y=Inverter Y
undo-bitfont-flip-x=Inverter X
undo-bitfont-move-down=Mover para baixo
undo-bitfont-move-up=Mover para cima
undo-bitfont-move-left=Mover para a esquerda
undo-bitfont-move-right=Mover para a direita
undo-bitfont-inverse=Inverter
undo-bitfont-clear=Limpar
undo-bitfont-edit=Editar
undo-bitfont-resize=Redimensionar
undo-delete=Excluir
undo-backspace=Backspace

undo-render_character=Renderizar caractere
undo-delete_character=Excluir caractere
undo-select=Selecionar
undo-plugin=Plugin { $title }

font_selector-ansi_font=ANSI
font_selector-library_font=BIBLIOTECA
font_selector-file_font=ARQUIVO
font_selector-sauce_font=SAUCE

select-palette-dialog-title=Selecionar paleta ({ $count } disponíveis)
select-palette-dialog-builtin_palette=INTEGRADA
select-palette-dialog-no-matching-palettes=Nenhuma paleta encontrada correspondente aos critérios de pesquisa.

autosave-dialog-title=Autosalvar
autosave-dialog-description=Foi encontrado um autosave para este arquivo.
autosave-dialog-question=Você quer usar o arquivo original ou carregar o autosave?
autosave-dialog-load_autosave_button=Carregar do autosave
autosave-dialog-discard_autosave_button=Descartar autosave

paste_mode-description=Você está agora no modo de colagem. Use a ferramenta de camada para adicionar ou ancorar a camada.
paste_mode-stamp=Carimbar
paste_mode-rotate=Rotacionar
paste_mode-flipx=Inverter X
paste_mode-flipy=Inverter Y
paste_mode-transparent=Transparente

ask_close_file_dialog-description=Você quer salvar as alterações feitas em { $filename }?
ask_close_file_dialog-subdescription=Suas alterações serão perdidas se você não salvar.
ask_close_file_dialog-dont_save_button=Não salvar
ask_close_file_dialog-save_button=Salvar

tab-context-menu-close=Fechar
tab-context-menu-close_others=Fechar outros
tab-context-menu-close_all=Fechar todos
tab-context-menu-copy_path=Copiar caminho

font-view-char_label=Caractere
font-view-ascii_label=ASCII
font-view-font_label=Fonte
font-view-font_page_label=Página de fonte:

font-editor-tile_area=Área de blocos
font-editor-clear=Limpar
font-editor-inverse=Inverter
font-editor-flip_x=Inverter X
font-editor-flip_y=Inverter Y

animation_editor_path_label=Caminho:
animation_editor_export_button=Exportar
animation_editor_ansi_label=Ansimation
animation_encoding_frame=Codificando quadro { $cur } de { $total }
animation_of_frame_count=de { $total }
animation_icy_play_note=Nota: Para reproduzir a animação no console/BBS ou conversão ANSI use:

new-file-template-cp437-title=ANSI CP437
new-file-template-cp437-description=
    Criar um novo arquivo ANSI de 16 cores DOS
    Limitado a 16 cores DOS e fonte Sauce, tem piscar (pode ser alternado)
new-file-template-ice-title=ANSI CP437 Ice
new-file-template-ice-description=
    Criar um novo arquivo ANSI de 16 cores ice DOS
    Limitado a 16 cores DOS e fonte Sauce, sem piscar (pode ser alternado)
new-file-template-xb-title=XB 16 Cores
new-file-template-xb-description=
    Criar um novo arquivo XB
    Paleta de 16 cores livre, 1 fonte, sem piscar (pode ser alternado)
new-file-template-xb-ext-title=Fonte Estendida XB
new-file-template-xb-ext-description=
    Criar um novo arquivo XB contendo duas fontes
    Paleta de 16 cores livre, 8 fg, 16 bg, 2 fontes, sem piscar
new-file-template-ansi-title=ANSI Moderno
new-file-template-ansi-description=
    Criar um novo arquivo ANSI sem restrições
    Paleta ilimitada, múltiplas fontes, piscar
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Criar um novo arquivo Atascii

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=Criar um novo arquivo FILE_ID.DIZ
new-file-template-ansimation-title=Ansimation
new-file-template-ansimation-description=Criar um novo arquivo de animação ANSI
new-file-template-bit_font-title=Fonte Bit
new-file-template-bit_font-description=Criar um novo arquivo de fonte bit
new-file-template-color_font-title=Fonte de Cor TDF
new-file-template-color_font-description=Criar uma nova fonte de cor TheDraw
new-file-template-block_font-title=Fonte de Bloco TDF
new-file-template-block_font-description=Criar uma nova fonte de bloco TheDraw
new-file-template-outline_font-title=Fonte de Contorno TDF
new-file-template-outline_font-description=Criar uma nova fonte de contorno TheDraw
new-file-template-ansimation-ui-label=
    Uma ansimation IcyDraw é um arquivo de texto lua descrevendo uma sequência de animação.
    Para uma descrição da sintaxe clique neste link:
new-file-template-bitfont-ui-label=
    Uma fonte bit é usada por computadores legados para exibir texto.

new-file-template-thedraw-ui-label=
    As fontes TheDraw são usadas para renderizar texto maior em editores ANSI.
    TheDraw definiu três tipos de fonte: Cor, Bloco e Contorno. 

    Um grande arquivo de fontes pode ser baixado de:

manage-font-dialog-title=Gerenciar fontes
manage-font-used_font_label=Fontes usadas
manage-font-copy_font_button=Copiar fonte
manage-font-copy_font_button-tooltip=Copia a fonte como sequência ANSI CTerm para a área de transferência. (para uso em BBS)
manage-font-remove_font_button=Remover
manage-font-used_label=usado
manage-font-not_used_label=não usado
manage-font-replace_label=Substituir uso com slot
manage-font-replace_font_button=Substituir
manage-font-change_font_slot_button=Alterar slot de fonte

palette_selector-dos_default_palette=VGA 16 cores
palette_selector-dos_default_low_palette=VGA 8 cores
palette_selector-c64_default_palette=Cores C64
palette_selector-ega_default_palette=EGA 64 cores
palette_selector-xterm_default_palette=Cores estendidas XTerm
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Extraído do buffer

tdf-editor-outline_preview_label=Pré-visualização do glifo de contorno
tdf-editor-draw_bg_checkbox=Usar fundo
tdf-editor-clone_button=Clonar
tdf-editor-font_name_label=Nome da fonte:
tdf-editor-spacing_label=Espaçamento:
tdf-editor-no_font_selected_label=Nenhuma fonte selecionada
tdf-editor-font_type_label=Tipo de fonte:
tdf-editor-font_type_color=Cor
tdf-editor-font_type_block=Bloco
tdf-editor-font_type_outline=Contorno
tdf-editor-clear_char_button=Limpar caractere
tdf-editor-cheat_sheet_key=Tecla
tdf-editor-cheat_sheet_code=Código
tdf-editor-cheat_sheet_res=Res

settings-heading=Configurações
settings-reset_button=Redefinir
settings-monitor-category=Monitor
settings-char-set-category=Conjuntos de caracteres
settings-font-outline-category=Contorno de fonte
settings-markers-guides-category=Marcadores e guias
settings-keybindings-category=Teclas
settings-reference-alpha=Alpha da imagem de referência
settings-raster-label=Cor da grade:
settings-alpha=alpha
settings-guide-label=Cor da guia:
settings-set-label=Definir { $set }
settings-key_filter_preview_text=Filtrar atalhos de teclas
settings-char_set_list_label=Conjuntos de caracteres:

edit-tag-title=Tag
edit-tag-filter=Filtrar tags
edit-tag-preview-label=Pré-visualização:
edit-tag-replacement-label=Substituição:
edit-tag-alignment-label=Alinhamento:
edit-tag-length-label=Comprimento:
edit-tag-alignment-left=Esquerda
edit-tag-alignment-right=Direita
edit-tag-alignment-center=Centro
edit-tag-placement-label=Posicionamento:
edit-tag-placement-in_line=Na linha
edit-tag-placement-after=Com GotoXY
edit-tag-role-label=Função:
edit-tag-role-displaycode=Código de exibição
edit-tag-role-hyperlink=Hiperlink

add_tag_tooltip=Adicionar tag
delete_tag_tooltip=Excluir tag


ask_unsaved_file_dialog-description=Deseja salvar as alterações nos seguintes {
    $number ->
        [1] arquivo?
        *[other] {$number} arquivos?
    }
ask_unsaved_file_dialog-subdescription=Suas alterações serão perdidas se você não salvar.
ask_unsaved_file_dialog-save_all_button=Salvar Tudo
ask_unsaved_file_dialog-dont_save_button=Não Salvar

# Save Changes Dialog (single file)
save-changes-title=Salvar alterações em "{ $filename }"?
save-changes-description=Suas alterações serão perdidas se você não salvá-las.

# Paste Tool Toolbar
paste-tool-stamp=Carimbar (S)
paste-tool-rotate=Girar (R)
paste-tool-flip-x=Espelhar X
paste-tool-flip-y=Espelhar Y
paste-tool-transparent=Transparente (T)
paste-tool-hint=Enter: Ancorar | Esc: Cancelar | Setas: Mover
# Animation Export Dialog
animation-export-format=Formato
animation-export-path=Exportar para
animation-export-no-path=Nenhum caminho selecionado
animation-export-success=Exportação concluída com sucesso
animation-export-exporting-frame=Exportando quadro { $current } / { $total }
animation-export-encoding=Codificando vídeo…
animation-export-cancelled=Exportação cancelada
animation-export-no-frames=Nenhum quadro para exportar
animation-export-failed=Falha na exportação: { $error }
