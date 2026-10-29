// 20 lines 13 code 6 comments 1 blanks
const a = <p className='x'>Don't panic</p>;
// comment
const b = (
  <div title="C:\">
    It's {/* comment */} "quoted" text // not a comment
    // still text

    {cond ? <b>can't</b> : 'str'}
    <img alt="a" />
  </div>
);
// comment
const c = a < b ? <></> : 2;
// comment
const d = <input
  // comment between attributes
  value="x"
/>;
// comment
