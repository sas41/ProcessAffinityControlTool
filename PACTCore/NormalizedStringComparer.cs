using System;
using System.Collections.Generic;
using System.Text;

namespace PACTCore
{
    class NormalizedStringComparer : StringComparer
    {
        public static NormalizedStringComparer Instance { get; } = new NormalizedStringComparer();

        private NormalizedStringComparer()
        {

        }

        public override int Compare(string x, string y)
        {
            return StringComparer.InvariantCultureIgnoreCase.Compare(x.Trim(), y.Trim());
        }

        public override bool Equals(string x, string y)
        {
            return StringComparer.InvariantCultureIgnoreCase.Equals(x.Trim(), y.Trim());
        }

        public override int GetHashCode(string obj)
        {
            return StringComparer.InvariantCultureIgnoreCase.GetHashCode(obj.Trim());
        }
    }
}
