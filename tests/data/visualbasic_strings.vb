' 12 lines 7 code 4 comments 1 blanks
Module M
    Sub Main()
        Dim a = "C:\"

        ' a backslash does not escape the quote before it
        Dim b = "say ""hi"" ' and this is not a comment"
        ' a comment
        Dim n = 7\"2"
        ' a backslash outside strings divides, and escapes nothing
    End Sub
End Module
