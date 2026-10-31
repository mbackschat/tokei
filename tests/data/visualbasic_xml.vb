' 16 lines 14 code 1 comments 1 blanks
Module M
    <Fact>
    Sub Main()
        Dim doc =
            <compilation>
                <file name="a.vb"><![CDATA[
' this is text, not a comment
Class C

End Class
]]></file>
            </compilation>
        Dim e = <a b="it's"><%= "x" %></a> ' a comment after the literal
    End Sub
End Module
