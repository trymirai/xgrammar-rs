#ifndef XGRAMMAR_RS_TESTING_DECL_HPP_
#define XGRAMMAR_RS_TESTING_DECL_HPP_

#include <string>

namespace xgrammar {
// Forward declaration of function implemented in libxgrammar.
// Converts Qwen XML tool-calling JSON schema to EBNF string.
std::string _QwenXMLToolCallingToEBNF(const std::string& schema);
}

#endif // XGRAMMAR_RS_TESTING_DECL_HPP_


