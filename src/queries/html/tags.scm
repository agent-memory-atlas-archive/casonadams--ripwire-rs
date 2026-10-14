; ripwire HTML tags — only index the document root element as a section,
; avoiding DOM element noise (<div>, <span>, <tr>, <td>) as class symbols.
(document
  (element
    (start_tag
      (tag_name) @name)) @definition.section)
