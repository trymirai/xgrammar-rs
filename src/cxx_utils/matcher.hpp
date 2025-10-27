#ifndef XGRAMMAR_RS_CXX_UTILS_MATCHER_H_
#define XGRAMMAR_RS_CXX_UTILS_MATCHER_H_

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline xgrammar::GrammarMatcher make_grammar_matcher(
    const xgrammar::CompiledGrammar& compiled
) {
  return xgrammar::GrammarMatcher(compiled);
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_MATCHER_H_
