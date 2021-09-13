# md-profiler, a tracing profiler for the Sega MegaDrive/Genesis

This program, meant to be used with [this fork](https://github.com/Tails8521/blastem) of BlastEm, helps you finding bottlenecks and having a better understanding of the performance of your games and ROM hacks. The currently supported assemblers, compilers and toolchains are asm68k, as, and gcc/SGDK.

![Screenshot](/screenshot.png)

# Basic usage

## Installation

Download [md-profiler](https://github.com/Tails8521/md-profiler/releases) and [this modified version of BlastEm](https://github.com/Tails8521/blastem/releases/tag/1.0.0), only Windows binaries are provided, other OS will have to compile from the source code.  

## Generating symbols

While not strictly required, symbols will allow you to make sense of the output of this program as they will allow you to see your labels and function names rather than raw addresses, you probably want to use symbols, it's pretty easy but the instructions differ slightly depending of what you use to build your game.

### Asm68k

When you build your game, the command should looks like this:
```
asm68k <OPTIONS...> mygame.asm, mygame.bin, mygame.sym, mygame.lst
```  
The important part is mygame.sym, this is the symbol file that will be generated. you may also want to add the ```/o v+``` switch to your options, this will tell asm68k to also list private labels in the symbol file (by default, only global labels are exported), optionally, you can also add the ```/o c+``` switch, this will treat the labels as case sensitive, by default they will be exported as lowercase unless you do this.

### AS

Add ```-g MAP``` to your build command, the .map file generated is your symbol file

### SGDK

SGDK default build scripts already generate symbols.txt which is your symbol file

## Recording a trace

Launch BlastEm with your game, when you want to record a trace, hit the 'u' key, this will open the BlastEm debugger console. Enter ```mdp <output.mdp>```  
This will resume your game, and generate the mdp file for profiling, when you are done, press 'u' again, and enter ```smdp``` in the console to stop the trace recording.  

## Generating the json trace

It's now time to use this program, the command is:
```
md-profiler -s <SYMBOLS> -i <INPUT> -o <OUTPUT>
```
 SYMBOLS is your symbol file, INPUT is the mdp file and OUTPUT is the json file this program will generate.

## Viewing the trace

You have several options:  
You can use https://ui.perfetto.dev/ in any browser, with the Open trace button in the top left, select your json file  
Or can use Google Chrome's chrome://tracing/ interface, press the Load button, on the top left and select your json file  

# Limitations and working around them

- By default, the profiler only follows explicit subroutine calls with JSR or BSR instructions, if you jump to, or fall trough subroutine code, it won't show that subroutine as being currently called. This is fixable however, even without changing your code, but it will require a bit of manual input on your part, see the Advanced usage section for more details.  
- C code with optimizations turned on tends to aggressively inline a vast amount of functions, and thus they don't appear in the graph. You can change the compiler options to make it inline less but keep in mind that builds with less inlining will not perform as well, and you may not get measurements that represent accurately how your optimized (with inlining) builds perform. The Advanced usage section contains a workaround that lets you profile inlined functions without affecting the generated code, at the cost of having to insert annotations manually in your source files.

# Advanced usage: Manual intervals  

## Writing your interval files

On top of automatically tracing subroutine calls and interrupts, you can also manually observe how long your code spends between two (or more) arbitrary points, you can specify these points by creating a text file where each line specify an interval and has this format:  
```
ENTRY POINTS,EXIT POINTS,OPTIONAL NAME, OPTIONAL CATEGORY
```  
Entry points and exit points can be labels or hex-formatted addresses, you can specify multiple entry points and/or multiple exit points by separating each with a semicolon ';', for instance:  
```
MySubroutineEntry1;MySubroutineEntry2;MySubroutineEntry3,MySubroutineExit1;MySubroutineExit2,MySubroutine
```  
An interval will start when any of the entry point is reached, and will end when any of the exit point is reached. If a label is both an entry point and and exit point for the same interval, it will stop the interval (if it was already started) and immediately start a new one.

If you don't specify a category, the interval will be stacked with others, automatically traced subroutines in the main thread. In case this is not what you want, you can name specify another, separate category to put that interval in, for instance:  
```
V_Int, WaitForVint, FrameTime, Frame time
```  
Will create the category "Frame time" and put it below the two default categories "Main thread" and "Interrupts"

## Passing the intervals to BlastEm

Now you need to use md-profiler in a special mode, which will generate a file to tell BlastEm which addresses it should pay attention to:  
```
md-profiler -m <INTERVALS> -s <SYMBOLS> -b <BREAKPOINTS OUTPUT FILE>
```  
In BlastEm use the mbp command to specify the breakpoint file location before recording the trace file with the mdp command:
```
mbp <BREAKPOINTS OUTPUT FILE>
mdp <output.mdp>
```  
And then you can use the mdp command to record the trace as usual, except you also specify the interval file:  
```
md-profiler -m <INTERVALS> -s <SYMBOLS> -i <INPUT> -o <OUTPUT>
```  

## Profiling inlined C functions

In your C code, you can use these macros  
```c
#define LABEL(name) asm volatile("mdp_label_" name "_%=: .global mdp_label_" name "_%=":);
#define FUNCTION_START(name) LABEL(name "_start");
#define FUNCTION_END(name)   LABEL(name "_end");
```
to define global labels and function start/end markers that the profiler can use

For instance, if myfunction gets inlined when compiling but you still want to profile it, you can do this: 

```c
s16 myfunction(s16 a) {
    FUNCTION_START("myfunction");
    if (a > 0) {
        FUNCTION_END("myfunction");
        return a+1;
    }
    if (a < 0) {
        FUNCTION_END("myfunction");
        return a-1;
    }
    FUNCTION_END("myfunction");
    return 0;
}
```

You need to add FUNCTION_START at the start of the function and the FUNCTION_END before each return statement (including the implicit one at the end of the function for void functions)  
In your interval file, you just specify the function name alone on a line, like so: 
```
myfunction
```

I know that ideally gcc would be able to automate this work automatically, I am aware of -finstrument-functions, but this isn't really what I want, I would need something like -finstrument-macros  
Anyway, if you are aware of a better way of doing this, please let me know.
