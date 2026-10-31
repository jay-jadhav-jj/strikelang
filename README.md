# StrikeLang
---  
> [!NOTE]
> This is not a release of the language, it still lacks many features.   
> This is just a view of the language so far to showcase.   
> What's missing:   
> - Dynamic String comparisons (== will not work with dynamic strings)
> - Dynamic booleans and integers
> - Functions that return values
> - Standard library (small std/io library is included though)
> - Floats (be careful with division; can lead to undefined errors as of now)

Also, the file extension for StrikeLang scripts MUST be .jcmd, or the compiler won't accept it!   
To compile StrikeLang code, use `strk c <PATH> <OUTPUT_NAME>`

---
## Features

With that being said, there are still many features implemented - enough to make programs.   
As of now, there are   
- Static strings, integers, and booleans
- Static evalutation
- Procedures
- Several compiler instructions
- Imports
- Error handling
- Dynamic strings
- Variables
- Scopes
- If-Elif-Else statements

Below are more details on the aforementioned.   

---
## Why StrikeLang?
I liked the simple syntax of scripting languages like Python, but for me, their speed was too slow.
You shouldn't have to sacrifice speed for ease of use. 
Since I know Assembly and Rust, I decided to create a compiled language that has simple and easy to understand syntax.
It is relatively simple compared to languages like C or C++, but runs at very fast speeds (try compiling a strikelang program and running it).
Also, I chose the name StrikeLang because I liked the character Striker Eureka from Pacific Rim.

---
## Why does it compile to NASM
It may seem like an odd design choice to compile to NASM rather than a heavily optimized language like C or Rust.
However, compiling straight to NASM gave the language a unique advantage: raw inline assembly.
StrikeLang allows raw inline assembly, meaning whatever you put in there goes directly into the compiled version.
You can't get that free control with C or Rust.
This is what allows StrikeLang to run extremely fast.
If you look at the standard library code, most of the procedures are just wrappers around inline assembly.
Because of this, you have to be careful not to make errors in inline assembly code - the compiler won't catch them.
Still, you can make many programs without touching most compiler instructions due to the standard library.

---
## Optimisation techniques
- Variables are inlined for the most part. Unless necessary, they aren't defined in assembly, and are replaced by constant values.
- Procedures are also inlined
- Heavy static interpretation - all operations that can be done at compile time will be done at compile time
- Here is an example of compile time optimisation
  ```
  x = 5 - 2 * 2
  y = x * x - 6
  exit y + 3
  # Equal to writing
  exit -2
  ```

---
## Syntax
- Procedures (functions that don't return values) are defined like this
  ```
  proc PROC_NAME(param_name TYPE, param_name TYPE, ...)
    (statements)...
  endpc
  ```
  For example, here is a snippet of the sayln procedure definition
  ```
  proc sayln(text STR)
    ...
  endpc
  ```
  Note that the only valid type specifiers right now are `STR` and `INT`
- Variables   
  Equivalent to python variable assignment/definition   
  `x = 1` can either be declaring `x` or assigning to it
- Compiler instructions
  The bridge between your code and assembly
  `INSTRUCTION_NAME{ arg, arg, ... }`
  For example, here is the syntax for calling `dynastr`
  `dynastr{ "address", 1024 }`
- If-Elif-Else
  All if statements must end with the endif keyword
  ```
  if CONDITION
      ...
  elif CONDITION
      ...
  else CONDITION
      ...
  endif
  ```
  Again, if the condition is static (which they have to be as of now), if statements are evaluated at compile time and have zero runtime cost
- Exit
  Exits the program with a given static integer exit code
  `exit 0` exits with code 0
- Use
  Imports code from another StrikeLang file (relative to where you are compiling from)
  `use "std/io"` imports the io library
- Procedure calls don't use brackets or commas. Arguments are simply whitespace separated
  For example, the `say_err(text STR, code INT)` can be called with `say_err "An error occured" 1`
- Comments
  `# Single line comments`
  ```
  /*

    Multi line comments


  */
  ```
- Lastly, there are no semicolons or indentation to separate statements and expressions. The parser figures it out.
  While you could do this
  ```
  use "std/io"

  sayln "Hello, World!"
  newln
  newln
  exit 0
  ```
  This would also be valid:
  ```
  use "std/io" sayln "Hello, World!" newln newln exit 0
  ```
---
## Compiler instructions
Here are the implemented compiler instructions which you will need to add functionality that isn't available in the standard library.   
It may be new at first, but the standard library uses most of these. You can check that code out to see these in use.   
- Inline assembly   
  Written like
  ```
  @{
    ; assembly code
  }
  ```
  This instruction does not take arguments and simply accepts any code given between the curly braces. Additionally, you can inject static string values with `{}`
  ```
  address = "fake_address"
  @{
    lea rdi, [rel {address}]
  }
  ```
- ReqStr   
  Because of static optimisation, no strings truly exist in compiled code until `reqstr` or `reqmutstr` is used.
  ReqStr takes a static string. If the string hasn't been generated yet, it creates a read-only string and returns its address as a static string.
  If it has been generated, it returns its address as a static string. Also, generates a constant at {address}_len that represents the size of the string.   
  Example:
  `text_addr = reqstr{ "Hello, World!" }`
- ReqMutStr   
  Creates a mutable string. Takes a static integer representing the amount of bytes that the string should be able to hold.
  4 bytes is around 1 character. Reqmutstr returns the address as a static string. Also, generates a dword at {address}_len that should store the current string length.   
  Example:
  `buf_addr = reqmutstr{ 1024 }`
- IsStatic    
  Takes any valid value, returns a static boolean that tells you whether the given value is static or not.
- DynaStr   
  The only way to create a string that isn't static. It takes an address (static string; preferably created through `reqmutstr`), and a static integer
  that represents the capacity of the address. This helps the compiler determine how to allocate buffers for string manipulation.
  Example:
  ```
  buf = reqmutstr{ 1024 }
  ... code that sets buf ...
  sayln "Hello, " + dynastr{ buf, 1024 }
  ```
- DynaAddr
  Returns the address of a given dynamic value, as a static string.
  Example:
  ```
  buf = dynastr{ reqmutstr{ 1024 } }

  sayln dynaaddr{ buf } # Same as the address given by reqmutstr
  ```
