using System;
using System.Collections.Generic;
using System.Text;

namespace PACTCore
{
    // This file contains a neat hack for a bug regarding collections
    // not having the correct comparer after deserialization.
    // Perhaps a JSON attribute to restore the correct comparer would be nice?
    // Either way, this hack feels cursed, but neat, but cursed.

    public class CaseInsensitiveDictionary<V> : Dictionary<string, V>
    {
        public CaseInsensitiveDictionary() : base(NormalizedStringComparer.Instance as IEqualityComparer<string>)
        {

        }
    }

    public class CaseInsensitiveHashSet : HashSet<string>
    {
        public CaseInsensitiveHashSet() : base(NormalizedStringComparer.Instance as IEqualityComparer<string>)
        {

        }
    }

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
