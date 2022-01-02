#!/usr/bin/env -S deno run --allow-read --allow-write

const EPILOGUE = 'llvm.donothing';

function processDefine(fun: string[]): string[] {
	let personality = Boolean(fun[0].match(/\bpersonality\b/));
	if(!personality)
		fun[0] = fun[0].replace(
			/{$/,
			'personality i32 ('
				+ 'i32, '
				+ 'i32, '
				+ 'i64, '
				+ '%"unwind::libunwind::_Unwind_Exception"*, '
				+ '%"unwind::libunwind::_Unwind_Context"*'
			+ ')* @rust_eh_personality $&',
		);

	let cleanup = false;
	for(let line = 0; line < fun.length; ++line)
		if(fun[line].match(/^  ret\b/)) {
			const label = 'ingerc' + line;
			fun.splice(
				line,
				0,
				'  invoke void @"'
					+ EPILOGUE
					+ '"() to label %'
					+ label
					+ ' unwind label %cleanup',
				label + ':',
			);
			line += 3;
		} else if(fun[line].match(/^cleanup:/))
			cleanup = true;

	if(!cleanup)
		fun.splice(
			fun.length - 1,
			0,
			'cleanup:',
			'  %ingerc = landingpad { i8*, i32 } cleanup',
			'  resume { i8*, i32 } %ingerc',
		);

	return fun;
}

if(Deno.args.length != 1) {
	console.log(
		'USAGE: ' + import.meta.url.replace(/.*\//, '') + ' <LLVM IR file>\n'
		+ '\n'
		+ 'Modify <LLVM IR file> to force llc to generate an LSDA for each function, even\n'
		+ 'those that statically cannot raise exceptions.'
	);
	Deno.exit(1);
}

const filename = Deno.args[0];
let ll = new TextDecoder().decode(Deno.readFileSync(filename));
let define = '\ndeclare void @"' + EPILOGUE + '"()';
if(ll.includes(define)) {
	console.log('We\'ve already processed this file!  Leaving it unchanged.');
	Deno.exit(2);
}

const funs = ll.split('\ndefine ').flatMap(function(elem) {
	const [head, tail] = elem.split('\n}\n');
	if(tail)
		return [('define ' + head + '\n}').split('\n'), tail];
	else
		return [head];
});

ll = funs.map(function(elem) {
	if(!Array.isArray(elem))
		return elem;

	return '\n' + processDefine(elem).join('\n') + '\n';
}).join('');
ll += define;
Deno.writeFileSync(filename, new TextEncoder().encode(ll));
