// 16 lines 9 code 7 comments 0 blanks
const f = <T,>(x: T) => x;
// comment
const g = <T extends object>(x: T): T => x;
// comment
const h = <Trans>Don't {x > 1 ? "it's" : `y`}</Trans>;
// comment
const i = list.map((x) => <li key={x}>{x}'s</li>);
// comment
const j = <Radio<Exclude<Device, "auto">>
  // comment between attributes
  name="x"
>
  it's text
</Radio>;
// comment
