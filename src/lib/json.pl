/* - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
   Written Apr 2021 by Aram Panasenco (panasenco@ucla.edu)
   Part of Scryer Prolog.
   
   json : Library for parsing and generating JSON-formatted data.
   
   BSD 3-Clause License
   
   Copyright (c) 2021, Aram Panasenco
   All rights reserved.
   
   Redistribution and use in source and binary forms, with or without
   modification, are permitted provided that the following conditions are met:
   
   * Redistributions of source code must retain the above copyright notice, this
     list of conditions and the following disclaimer.
   
   * Redistributions in binary form must reproduce the above copyright notice,
     this list of conditions and the following disclaimer in the documentation
     and/or other materials provided with the distribution.
   
   * Neither the name of the copyright holder nor the names of its
     contributors may be used to endorse or promote products derived from
     this software without specific prior written permission.
   
   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
   AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
   DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
   FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
   DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
   CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
   OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
   OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
- - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */

:- module(json, [
                 json_whitespace//0,
                 json_string//1
                ]).

:- use_module(library(assoc)).
:- use_module(library(clpz)).
:- use_module(library(dcgs)).
:- use_module(library(dif)).
:- use_module(library(lists)).
:- use_module(library(pairs)).
:- use_module(library(pure)).
:- use_module(library(reif)).

json_whitespace --> code("\t\n\r "), json_whitespace.
json_whitespace --> "".

escape_map([
    34 - 34,  /* "  - " */
    92 - 92,  /* \  - \ */
    47 - 47,  /* /  - / */
    8  - 98,  /* \b - b */
    12 - 102, /* \f - f */
    10 - 110, /* \n - n */
    13 - 114, /* \r - r */
    9  - 116  /* \t - t */
]).

right_string([C | Tail]) -->
    {
        escape_map(EscapeMap),
        pairs_keys(EscapeMap, Escapes),
        if_(code_incl_excl_t(C, [alphanumeric, ascii_graphic, chars(" ")], [codes(Escapes)]),
            JsonCodes = [C],
            (
                char_code('\\', Backslash),
                if_(memberd_t(C-PrintCode, EscapeMap),
                    JsonCodes = [Backslash, PrintCode],
                    (
                        char_code('u', U),
                        integer_hexcodes(C, [H1, H2, H3, H4]),
                        JsonCodes = [Backslash, U, H1, H2, H3, H4]
                    )
                )
            )
        )
    },
    JsonCodes,
    right_string(Tail).
right_string("") --> codes("\"").
json_string(String) --> codes("\""), right_string(String).
